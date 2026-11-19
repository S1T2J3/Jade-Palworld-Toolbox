//! Private pipes to the installed assistant module. Only this module starts it.
use eframe::egui;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_OUTPUT: usize = 4 * 1024 * 1024;
const INTERRUPTED: &str = "内核进程已中断。请重发问题以开始新的内核会话。";

#[derive(Clone, Debug, Deserialize)]
pub struct AssistantReply {
    pub text: String,
    pub links: Vec<(String, String)>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum QueryPlan {
    Evidence { answer: AssistantReply, #[serde(default)] complete: bool },
    Search {
        terms: Vec<String>,
        clarification: String,
    },
    Clarify {
        question: String,
    },
    Outside,
}

type Reply = Result<Value, String>;

struct Job {
    payload: Value,
    finish: Box<dyn FnOnce(Reply) + Send>,
}

#[derive(Default)]
struct Running {
    stopped: bool,
    child: Option<Child>,
}

impl Running {
    fn stop_child(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Each lane serializes requests so related turns see a consistent conversation.
/// Slow generation runs in a separate process and cannot block handbook lookups.
struct Lane {
    jobs: mpsc::SyncSender<Job>,
    running: Arc<Mutex<Running>>,
}

impl Lane {
    fn new(executable: PathBuf, resources: PathBuf, keep_session: bool) -> Self {
        let timeout = Duration::from_secs(if keep_session { 30 } else { 190 });
        let (jobs, next) = mpsc::sync_channel::<Job>(16);
        let running = Arc::new(Mutex::new(Running::default()));
        let state = Arc::clone(&running);
        thread::spawn(move || {
            let mut pipe = None;
            while let Ok(job) = next.recv() {
                if state.lock().unwrap().stopped {
                    break;
                }
                if pipe.is_none() {
                    match start(&executable, &resources, &state) {
                        Ok(input) => pipe = Some(input),
                        Err(reason) => {
                            (job.finish)(Err(reason));
                            continue;
                        }
                    }
                }
                let (answer, received) = mpsc::channel();
                let exchange = pipe.as_ref().unwrap().send((job.payload, answer));
                let response = if exchange.is_ok() {
                    received.recv_timeout(timeout).map_err(|error| match error {
                        mpsc::RecvTimeoutError::Timeout => {
                            "内核响应超时，已结束这次内核会话。".to_owned()
                        }
                        mpsc::RecvTimeoutError::Disconnected => INTERRUPTED.to_owned(),
                    })
                } else {
                    Err(INTERRUPTED.to_owned())
                };
                if !keep_session || !matches!(&response, Ok(Ok(_))) {
                    state.lock().unwrap().stop_child();
                    pipe = None;
                }
                let result = response.and_then(|line| line).and_then(reply_value);
                (job.finish)(result);
            }
            state.lock().unwrap().stop_child();
        });
        Self { jobs, running }
    }
}

impl Drop for Lane {
    fn drop(&mut self) {
        let mut running = self.running.lock().unwrap();
        running.stopped = true;
        // Unblock a pipe operation even when a provider was still generating.
        running.stop_child();
    }
}

type PipeCall = (Value, mpsc::Sender<Reply>);

fn start(
    executable: &Path,
    resources: &Path,
    state: &Mutex<Running>,
) -> Result<mpsc::Sender<PipeCall>, String> {
    let verified = crate::assistant_modules::verified_executable(executable)?;
    let mut command = Command::new(executable);
    command
        .arg("--resources")
        .arg(resources)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut running = state.lock().unwrap();
    if running.stopped {
        return Err(INTERRUPTED.into());
    }
    let mut child = command
        .spawn()
        .map_err(|_| "无法启动已安装的帕鲁助手内核。".to_owned())?;
    drop(verified);
    let mut input = child.stdin.take().expect("piped stdin");
    let mut output = BufReader::new(child.stdout.take().expect("piped stdout"));
    running.child = Some(child);
    drop(running);
    let (send, requests) = mpsc::channel::<PipeCall>();
    thread::spawn(move || {
        while let Ok((request, done)) = requests.recv() {
            let result = exchange(&mut input, &mut output, &request);
            let broken = result.is_err();
            let _ = done.send(result);
            if broken {
                break;
            }
        }
    });
    Ok(send)
}

fn exchange(input: &mut ChildStdin, output: &mut BufReader<ChildStdout>, request: &Value) -> Reply {
    let data = serde_json::to_vec(request).map_err(|_| "无法准备内核请求。".to_owned())?;
    if data.len() >= MAX_INPUT {
        return Err("本次世界记录超过内核请求大小限制。".into());
    }
    input
        .write_all(&data)
        .and_then(|_| input.write_all(b"\n"))
        .and_then(|_| input.flush())
        .map_err(|_| INTERRUPTED.to_owned())?;
    let mut line = Vec::new();
    output
        .take((MAX_OUTPUT + 1) as u64)
        .read_until(b'\n', &mut line)
        .map_err(|_| INTERRUPTED.to_owned())?;
    if line.is_empty() {
        return Err(INTERRUPTED.into());
    }
    if line.len() > MAX_OUTPUT || !line.ends_with(b"\n") {
        return Err("内核响应过大或不完整。".into());
    }
    serde_json::from_slice(&line).map_err(|_| "无法解析内核响应。".into())
}

fn reply_value(mut value: Value) -> Reply {
    if let Some(message) = value.get("error").and_then(Value::as_str) {
        return Err(message.chars().take(512).collect());
    }
    value
        .as_object_mut()
        .and_then(|obj| obj.remove("ok"))
        .ok_or_else(|| "已安装的内核未返回可识别的结果。".to_owned())
}

pub struct KernelClient {
    executable: PathBuf,
    resources: PathBuf,
    handbook: Mutex<Option<Lane>>,
    provider: Mutex<Option<Lane>>,
}

impl KernelClient {
    pub fn new(executable: PathBuf, resources: PathBuf) -> Self {
        Self {
            executable,
            resources,
            handbook: Mutex::default(),
            provider: Mutex::default(),
        }
    }

    /// Drop both subprocesses and their queues. A new child is started on demand.
    pub fn reset(&mut self) {
        self.handbook.get_mut().unwrap().take();
        self.provider.get_mut().unwrap().take();
    }

    pub fn request<T: DeserializeOwned + Send + 'static>(
        &self,
        ctx: &egui::Context,
        request: Value,
    ) -> mpsc::Receiver<Result<T, String>> {
        let (send, recv) = mpsc::channel();
        let ctx = ctx.clone();
        let finish = Box::new(move |result: Reply| {
            let value = result.and_then(|v| {
                serde_json::from_value(v).map_err(|_| "内核返回的数据与当前版本不兼容。".to_owned())
            });
            let _ = send.send(value);
            ctx.request_repaint();
        });
        let keep_session = matches!(request["op"].as_str(), Some("ask" | "retrieve"));
        let slot = if keep_session {
            &self.handbook
        } else {
            &self.provider
        };
        let mut lane = slot.lock().unwrap();
        let lane = lane.get_or_insert_with(|| {
            Lane::new(
                self.executable.clone(),
                self.resources.clone(),
                keep_session,
            )
        });
        if let Err(error) = lane.jobs.try_send(Job {
            payload: request,
            finish,
        }) {
            let (job, message) = match error {
                mpsc::TrySendError::Full(job) => (job, "内核仍在处理前面的问题，请稍后重试。"),
                mpsc::TrySendError::Disconnected(job) => (job, INTERRUPTED),
            };
            (job.finish)(Err(message.into()));
        }
        recv
    }
}
