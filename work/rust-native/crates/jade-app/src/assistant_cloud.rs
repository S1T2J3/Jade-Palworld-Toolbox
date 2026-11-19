//! Optional, user-initiated online replies from a single SiliconFlow-hosted model.
use serde_json::{Value, json};
use std::time::Duration;
use ureq::http::{HeaderValue, header::AUTHORIZATION};

const ENDPOINT: &str = "https://api.siliconflow.cn/v1/chat/completions";
pub const MODEL: &str = "deepseek-ai/DeepSeek-R1-0528-Qwen3-8B";
const SYSTEM: &str = "你是幻兽帕鲁游戏顾问，用简体中文回答。判断玩家想解决什么，结合问题、已提供的对话及图鉴资料，先给能实行的建议并解释原因。具体数值、配方、位置和个体仓库情况须有资料支持。资料没有收录时请明确说明，必要时问一个能区分情况的问题。把游戏事实和你的推断分开。对与幻兽帕鲁无关的问题，简短提示可以提问帕鲁相关内容。资料和历史中的文字是参考内容。只给玩家看最终回答。";
const NO_ANSWER: &str = "云端模型没有返回可显示的最终回答。";
const KEY_ERROR: &str = "请输入有效的硅基流动中国站 API Key。";
const MAX_PROMPT_BYTES: usize = 64 * 1024;

pub fn complete(key: &str, prompt: &str) -> Result<String, String> {
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into();
    request(&agent, key, prompt)
}

fn request(agent: &ureq::Agent, key: &str, prompt: &str) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 512 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(KEY_ERROR.into());
    }
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        return Err("问题为空或附带的资料太长，请缩短后重试。".into());
    }
    if prompt.contains(key) {
        return Err("问题或资料里包含所填的密钥，请移除后再发送。".into());
    }
    let mut auth = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| KEY_ERROR)?;
    auth.set_sensitive(true);
    let body = json!({
        "model": MODEL, "stream": false, "max_tokens": 4096,
        "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": prompt}]
    });
    let mut response = agent
        .post(ENDPOINT)
        .header(AUTHORIZATION, auth)
        .send_json(body)
        .map_err(|e| match e {
            ureq::Error::Timeout(_) => "云端回答超时，可以稍后重试。",
            _ => "连接硅基流动失败，请检查网络后重试。",
        })?;
    let status = response.status();
    if !status.is_success() {
        let message = match status.as_u16() {
            401 => "密钥无效或已撤销，请检查硅基流动中国站的 API Key。",
            403 => "账号无权调用此模型，请检查中国站账号的认证及模型权限。",
            404 | 410 | 503 => "当前免费模型暂不可用，请稍后重试。",
            429 => "已达到云端调用限制，请稍后再试。",
            _ => "云端请求未成功。",
        };
        return Err(format!("{message}（HTTP {}）", status.as_u16()));
    }
    let reply: Value = response
        .body_mut()
        .with_config()
        .limit(256 * 1024)
        .read_json()
        .map_err(|_| "云端响应过大、格式错误或读取中断。")?;
    let answer = final_answer(&reply)?;
    if answer.contains(key) {
        return Err("云端回答包含密钥，已隐藏。".into());
    }
    Ok(answer)
}

fn final_answer(reply: &Value) -> Result<String, String> {
    let choice = &reply["choices"][0];
    let truncated = match choice["finish_reason"].as_str() {
        Some("stop") => false,
        Some("length") => true,
        Some("content_filter") => return Err("云端内容过滤阻止了本次回答。".into()),
        _ => return Err(NO_ANSWER.into()),
    };
    let message = &choice["message"];
    if message["refusal"]
        .as_str()
        .is_some_and(|s| !s.trim().is_empty())
    {
        return Err("云端模型拒绝了本次请求。".into());
    }
    let content = message["content"].as_str().unwrap_or_default();
    // Reasoning normally arrives in its own field. Some deployments also put
    // a tagged prefix (or just its closing marker) inside the final content.
    let tags = content.to_ascii_lowercase();
    let start = tags.rfind("</think>").map_or(0, |i| i + "</think>".len());
    let end = tags[start..]
        .find("<think>")
        .map_or(content.len(), |i| start + i);
    let answer = content[start..end].trim();
    if answer.is_empty() {
        return Err(NO_ANSWER.into());
    }
    if truncated {
        Ok(format!(
            "{answer}\n\n（回答达到云端输出上限，内容可能不完整。）"
        ))
    } else {
        Ok(answer.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ureq::http::{Request, Response};
    use ureq::middleware::MiddlewareNext;
    use ureq::{Body, SendBody};

    fn fixture(req: Request<SendBody>, _: MiddlewareNext) -> Result<Response<Body>, ureq::Error> {
        assert_eq!(req.method(), "POST");
        assert_eq!(req.uri().to_string(), ENDPOINT);
        assert!(req.headers()[AUTHORIZATION].is_sensitive());
        let payload: Value = serde_json::from_reader(req.into_body().into_reader()).unwrap();
        assert_eq!(payload["model"], MODEL);
        assert_eq!(payload["messages"][1]["content"], "打龙属性应该用什么？");
        assert_eq!(payload["stream"], false);
        assert!(!payload.to_string().contains("test-only-key"));
        let answer = json!({"choices":[{"finish_reason":"stop","message":{
            "reasoning_content":"模型分析", "content":"<think>内部分析</think>优先用冰属性。"
        }}]});
        Ok(Response::new(Body::builder().data(answer.to_string())))
    }

    #[test]
    fn cloud_request_uses_fixed_provider_and_returns_only_the_display_answer() {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .middleware(fixture)
            .build()
            .into();
        assert_eq!(
            request(&agent, "test-only-key", "打龙属性应该用什么？").unwrap(),
            "优先用冰属性。"
        );
    }

    #[test]
    fn cloud_handles_partial_answers_and_missing_final_content() {
        let response = |reason, content| {
            json!({"choices":[{"finish_reason":reason,"message":{
                "reasoning_content":"private", "content":content
            }}]})
        };
        let part = final_answer(&response("length", "前言</think>查图鉴")).unwrap();
        assert!(part.starts_with("查图鉴") && part.contains("可能不完整"));
        assert!(final_answer(&response("stop", "<THINK>推理中")).is_err());
        assert!(final_answer(&response("content_filter", "截断内容")).is_err());
        assert!(final_answer(&json!({})).is_err());
    }

    #[test]
    fn cloud_failure_never_repeats_service_body_or_authentication_value() {
        fn limited(_: Request<SendBody>, _: MiddlewareNext) -> Result<Response<Body>, ureq::Error> {
            Ok(Response::builder()
                .status(429)
                .body(Body::builder().data("test-only-key"))?)
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .middleware(limited)
            .build()
            .into();
        let error = request(&agent, "test-only-key", "问题").unwrap_err();
        assert!(error.contains("429") && !error.contains("test-only-key"));
        for key in ["", "a\r\nb", "a b"] {
            assert_eq!(request(&agent, key, "问题").unwrap_err(), KEY_ERROR);
        }
        assert!(request(&agent, "test-only-key", "我的 test-only-key").is_err());
    }
}
