//! The optional assistant runs behind the desktop application's private stdin/stdout pipes.
mod assistant_breeding;
mod assistant_cloud;
mod assistant_compare;
mod assistant_data;
mod assistant_discovery;
mod assistant_elements;
mod assistant_engine;
mod assistant_equipment;
mod assistant_intent;
mod assistant_local;
mod assistant_recommend;
mod assistant_survival;
#[path = "assistant_kernel_data.rs"]
mod data;
#[cfg(test)]
mod store;

use assistant_engine::{AssistantReply, Conversation};
use assistant_discovery::environment::{self, SpawnScope};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_OUTPUT: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Request {
    Ask {
        question: String,
        world: Value,
    },
    Retrieve {
        terms: Vec<String>,
        #[serde(default)]
        question: String,
        #[serde(default)]
        translation: Option<String>,
    },
    Models,
    Plan {
        model: String,
        question: String,
        history: Vec<(String, String)>,
    },
    Compose {
        model: String,
        question: String,
        facts: String,
        history: Vec<(String, String)>,
    },
    Cloud {
        key: String,
        prompt: String,
    },
    Reset,
}

fn answer_value(answer: AssistantReply) -> Value {
    json!({"text": answer.text, "links": answer.links, "suggestions": answer.suggestions})
}

fn advice(text: &str) -> Value {
    json!({"text":text,"context":text,"direct_result":true,"links":[],"suggestions":[]})
}

struct Worker {
    resources: PathBuf,
    data: Option<data::Data>,
    conversation: Conversation,
    spawn_scope: SpawnScope,
}

impl Worker {
    fn new(resources: PathBuf) -> Self {
        Self {
            resources,
            data: None,
            conversation: Conversation::default(),
            spawn_scope: Default::default(),
        }
    }

    fn dispatch(&mut self, request: Request, evidence_trace: &mut Value) -> Result<Value, String> {
        match request {
            Request::Ask { question, world } => {
                if !world.is_object()
                    || question.trim().is_empty()
                    || question.chars().count() > 512
                {
                    return Err("问题或当前世界资料无效。".into());
                }
                let q = assistant_engine::field_text(&question);
                if let Some(help) = environment::troubleshooting(&q) {
                    self.conversation = Conversation::default();
                    self.spawn_scope = Default::default();
                    return Ok(advice(help));
                }
                let data = load_once(&mut self.data, &self.resources)?;
                let reply = self.conversation.ask_world(&question, data, &world);
                let habitat = environment::is_habitat(&reply);
                if self.spawn_scope.randomized_habitat(&q, world["id"].as_str(), habitat) {
                    return Ok(advice(environment::spawn_help(&q)));
                }
                let mut answer = answer_value(reply);
                answer["context"] = json!(self.conversation.model_context());
                answer["direct_result"] = json!(self.conversation.direct_result());
                Ok(answer)
            }
            Request::Retrieve { terms, question, translation } => {
                let data = load_once(&mut self.data, &self.resources)?;
                Ok(assistant_discovery::retrieve_planned(data, &terms, &question, translation.as_deref())
                    .map(answer_value)
                    .unwrap_or(Value::Null))
            }
            Request::Models => assistant_local::list_models().map(|list| json!(list)),
            Request::Plan {
                model,
                question,
                history,
            } => {
                use assistant_local::QueryPlan::*;
                let data = load_once(&mut self.data, &self.resources)?;
                let structured = assistant_discovery::team_evidence(data, &question)
                    .or_else(|| history.is_empty().then(|| assistant_discovery::evidenced_relation(data,&question)).flatten());
                let has_table = structured.is_some();
                let original = structured.or_else(|| assistant_discovery::retrieve_for_question(data, &[], &question));
                let plan = assistant_local::plan(&model, &question, &history, original.as_ref().map(|r| r.text.as_str()))?;
                Ok(match plan {
                    Search {
                        terms,
                        translation,
                        clarification,
                        field,
                    } => {
                        let focused = if let Some(field) = field {
                            assistant_engine::model_field_evidence(data, field, &question, &history)
                        } else { None };
                        if let (Some(mut answer), Some(selected_field)) = (focused, field) {
                            if environment::is_habitat(&answer) {
                                // The desktop model lane has its own worker. Its only
                                // record of a prior random-world lookup is the history.
                                let mut scope = SpawnScope::default();
                                if let Some((prior, prior_reply)) = history.last() {
                                    let prior = assistant_engine::field_text(prior);
                                    scope.randomized_habitat(&prior, None, prior_reply.contains(environment::spawn_help(&prior)));
                                }
                                let q = assistant_engine::field_text(&question);
                                if scope.randomized_habitat(&q, None, true) {
                                    return Ok(json!({"kind":"evidence","answer":advice(environment::spawn_help(&q)),"field":field,"complete":true}));
                                }
                            }
                            let complete = selected_field.can_answer_directly(&question);
                            if !complete && !has_table {
                                selected_field.label_evidence(&mut answer);
                            }
                            let answer = match original {
                                Some(table) if has_table && !complete => table,
                                context if !complete => {
                                    let retrieved = translation.as_deref().and_then(|q|
                                        assistant_discovery::retrieve_planned(data, &terms, &question, Some(q))).or(context);
                                    assistant_discovery::combine(retrieved.into_iter().chain([answer.clone()])).unwrap_or(answer)
                                },
                                _ => answer,
                            };
                            json!({"kind":"evidence", "answer":answer_value(answer), "field":field, "complete":complete, "translation":translation, "terms":terms})
                        } else if let Some(answer) = if has_table { original } else {
                            assistant_discovery::retrieve_planned(data, &terms, &question, translation.as_deref()).or(original)
                        } {
                            json!({"kind":"evidence","answer":answer_value(answer),"complete":false,"translation":translation,"terms":terms})
                        } else {
                            json!({"kind":"search", "terms":terms, "clarification":clarification,"translation":translation})
                        }
                    },
                    Clarify(followup) => match original {
                        Some(answer) => json!({"kind":"evidence","answer":answer_value(answer),"complete":false}),
                        None => json!({"kind":"clarify","question":followup,"needs_player_input":true}),
                    },
                    Outside => match original {
                        Some(answer) if has_table => json!({"kind":"evidence","answer":answer_value(answer),"complete":false}),
                        _ => json!({"kind":"outside"}),
                    },
                })
            }
            Request::Compose {
                model,
                question,
                facts,
                history,
            } => {
                let data = load_once(&mut self.data, &self.resources)?;
                let partner = assistant_engine::HandbookField::partner_excerpt(data, &question, &history, &facts);
                let relation = if history.is_empty() {assistant_discovery::evidenced_relation(data,&question).map(|r|r.text)}else{None};
                let passages = if partner.is_none() && history.is_empty() {
                    assistant_discovery::offered_passages(data, &question, &facts)
                } else { Vec::new() };
                assistant_local::compose(data, &model, &question, &facts, &history, partner.as_deref(), &passages, evidence_trace, || {
                    assistant_engine::HandbookField::verified_excerpt(data, &question, &history, &facts)
                        .or_else(|| relation.clone().filter(|body| facts.contains(body)))
                        .or_else(|| assistant_discovery::team_evidence(data, &question)
                            .or_else(|| assistant_discovery::survival_guide_evidence(&question))
                            .map(|r| r.text).filter(|text| facts.contains(text.trim())))
                }).map(|answer| {
                    // A complete table row is already player-readable. Keep its
                    // units and labels when it is the sole source, e.g. reward
                    // probability is not expedition success probability.
                    if let Some(row) = relation.filter(|body| body.trim() == facts.trim()) {
                        evidence_trace["outcome"] = json!("sole_structured_record");
                        Value::String(row)
                    } else {
                        Value::String(answer)
                    }
                })
            },
            Request::Cloud { key, prompt } => {
                assistant_cloud::complete(&key, &prompt).map(Value::String)
            }
            Request::Reset => {
                self.conversation = Conversation::default();
                self.spawn_scope = Default::default();
                Ok(json!(true))
            }
        }
    }
}

fn load_once<'a>(slot: &'a mut Option<data::Data>, root: &Path) -> Result<&'a data::Data, String> {
    if slot.is_none() {
        *slot = Some(data::Data::load(root)?);
    }
    Ok(slot.as_ref().expect("handbook loaded"))
}

fn serve(input: &mut impl BufRead, output: &mut impl Write, worker: &mut Worker) -> io::Result<()> {
    loop {
        let mut line = Vec::new();
        // Take at most one wire frame plus one byte; an oversized frame ends this
        // pipe rather than re-interpreting part of it as a subsequent request.
        use io::Read;
        let size = input
            .take(MAX_INPUT as u64 + 2)
            .read_until(b'\n', &mut line)?;
        if size == 0 {
            return Ok(());
        }
        if size > MAX_INPUT + 1 || !line.ends_with(b"\n") {
            serde_json::to_writer(&mut *output, &json!({"error":"内核请求过大或不完整。"}))?;
            output.write_all(b"\n")?;
            output.flush()?;
            return Ok(());
        }
        let mut evidence_trace = Value::Null;
        let result = serde_json::from_slice::<Request>(&line)
            .map_err(|_| "内核请求格式有误，或调用了不支持的操作。".to_owned())
            .and_then(|request| worker.dispatch(request, &mut evidence_trace));
        let mut response = match result {
            Ok(value) => json!({"ok":value}),
            Err(message) => json!({"error":message}),
        };
        if !evidence_trace.is_null() {
            response["evidenceTrace"] = evidence_trace;
        }
        let mut encoded = serde_json::to_vec(&response)?;
        if encoded.len() + 1 > MAX_OUTPUT {
            encoded = serde_json::to_vec(&json!({"error":"内核回答太长，请缩小查询范围。"}))?;
        }
        output.write_all(&encoded)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
}

fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--resources")) {
        return std::process::ExitCode::from(2);
    }
    let Some(path) = args.next() else {
        return std::process::ExitCode::from(2);
    };
    if args.next().is_some() {
        return std::process::ExitCode::from(2);
    }
    let mut worker = Worker::new(PathBuf::from(path));
    match serve(
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        &mut worker,
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::from(1),
    }
}
