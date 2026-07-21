//! HTTP 调试落盘：由环境变量开启，按会话记录请求头、请求体与流式重组响应。
//!
//! 环境变量:
//! - `SAI_DEBUG_HTTP=1|true|yes|on`：开启
//! - `SAI_DEBUG_HTTP_SESSION=<session_id>`：可选，仅记录指定会话
//!
//! 输出目录: `{cache_dir}/debug-http/<session_id>/<timestamp>_<seq>/`

use crate::llm::ChatResult;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use reqwest::header::HeaderMap;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

thread_local! {
    /// 当前线程关联的会话 ID（由 Agent 在调用 LLM 前设置）。
    static CURRENT_SESSION_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);
static PATH_HINT_PRINTED: OnceLock<()> = OnceLock::new();

/// 从环境变量解析得到的调试配置。
#[derive(Debug, Clone)]
pub struct HttpDebugConfig {
    /// 根目录：cache_dir/debug-http
    root: PathBuf,
    /// 仅记录该会话；空表示全部会话
    session_filter: Option<String>,
}

impl HttpDebugConfig {
    /// 从环境变量与路径构造配置；未开启时返回 `None`。
    ///
    /// 参数:
    /// - `paths`: Sai 路径
    ///
    /// 返回:
    /// - 开启时返回配置
    pub fn from_env(paths: &SaiPaths) -> Option<Self> {
        let flag = std::env::var("SAI_DEBUG_HTTP").ok()?;
        if !is_truthy(&flag) {
            return None;
        }
        let session_filter = std::env::var("SAI_DEBUG_HTTP_SESSION")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Some(Self {
            root: paths.cache_dir.join("debug-http"),
            session_filter,
        })
    }

    /// 当前会话是否应落盘。
    ///
    /// 参数:
    /// - `session_id`: 当前会话
    ///
    /// 返回:
    /// - 是否记录
    fn should_record(&self, session_id: &str) -> bool {
        match &self.session_filter {
            Some(filter) => filter == session_id,
            None => true,
        }
    }
}

/// 在作用域内绑定会话 ID，Drop 时恢复。
pub struct SessionGuard {
    previous: Option<String>,
}

impl SessionGuard {
    /// 绑定当前线程的会话 ID。
    ///
    /// 参数:
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 作用域守卫
    pub fn new(session_id: &str) -> Self {
        let previous = CURRENT_SESSION_ID.with(|cell| cell.borrow().clone());
        CURRENT_SESSION_ID.with(|cell| {
            *cell.borrow_mut() = Some(session_id.to_string());
        });
        Self { previous }
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        CURRENT_SESSION_ID.with(|cell| {
            *cell.borrow_mut() = self.previous.take();
        });
    }
}

/// 单次 HTTP 请求的落盘记录器。
pub struct HttpDebugRecorder {
    dir: PathBuf,
    stream_buf: String,
}

impl HttpDebugRecorder {
    /// 若调试开启且会话匹配，创建目录并写入请求侧文件。
    ///
    /// 参数:
    /// - `config`: 调试配置
    /// - `method`: HTTP 方法
    /// - `url`: 请求 URL
    /// - `provider_id`: provider 标识
    /// - `protocol`: 协议标签
    /// - `request_headers`: 已脱敏的请求头 (name, value)
    /// - `body`: 请求 JSON 体
    ///
    /// 返回:
    /// - 记录器；会话被过滤或不匹配时 `None`
    pub fn start(
        config: &HttpDebugConfig,
        method: &str,
        url: &str,
        provider_id: &str,
        protocol: &str,
        request_headers: &[(String, String)],
        body: &Value,
    ) -> Result<Option<Self>> {
        let session_id = current_session_id().unwrap_or_else(|| "unknown".to_string());
        if !config.should_record(&session_id) {
            return Ok(None);
        }
        // 1. 会话目录 + 单调序号时间戳
        let seq = REQUEST_SEQ.fetch_add(1, Ordering::Relaxed);
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3f");
        let dir = config
            .root
            .join(&session_id)
            .join(format!("{stamp}_{seq:04}"));
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create http debug dir {}", dir.display()))?;
        // 2. 首次落盘时提示路径（仅一次）
        PATH_HINT_PRINTED.get_or_init(|| {
            eprintln!(
                "[sai] HTTP debug enabled; writing under {}",
                config.root.display()
            );
        });
        // 3. 写入 meta / 请求头 / 请求体
        let meta = json!({
            "session_id": session_id,
            "method": method,
            "url": url,
            "provider_id": provider_id,
            "protocol": protocol,
            "started_at": chrono::Utc::now().to_rfc3339(),
            "dir": dir.display().to_string(),
        });
        write_json(&dir.join("meta.json"), &meta)?;
        write_headers_file(&dir.join("request_headers.txt"), request_headers)?;
        write_json(&dir.join("request_body.json"), body)?;
        Ok(Some(Self {
            dir,
            stream_buf: String::new(),
        }))
    }

    /// 记录响应状态与响应头（值中敏感字段脱敏）。
    ///
    /// 参数:
    /// - `status`: HTTP 状态码
    /// - `headers`: 响应头
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_response_headers(&self, status: u16, headers: &HeaderMap) -> Result<()> {
        let mut lines = vec![format!("status: {status}")];
        for (name, value) in headers.iter() {
            let value = value.to_str().unwrap_or("<binary>");
            lines.push(format!(
                "{}: {}",
                name.as_str(),
                redact_header_value(name.as_str(), value)
            ));
        }
        fs::write(
            self.dir.join("response_headers.txt"),
            lines.join("\n") + "\n",
        )
        .with_context(|| {
            format!(
                "failed to write response headers {}",
                self.dir.join("response_headers.txt").display()
            )
        })?;
        Ok(())
    }

    /// 追加一条已解码的 SSE 文本行（含 `data:` 行）。
    ///
    /// 参数:
    /// - `line`: 完整一行（不含换行）
    pub fn append_stream_line(&mut self, line: &str) {
        self.stream_buf.push_str(line);
        self.stream_buf.push('\n');
    }

    /// 流式结束：写出原始 SSE 与重组后的非流式结果。
    ///
    /// 参数:
    /// - `result`: 应用层解析后的完整结果
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn finish_ok(&self, result: &ChatResult) -> Result<()> {
        fs::write(self.dir.join("response_stream.sse"), &self.stream_buf).with_context(|| {
            format!(
                "failed to write response stream {}",
                self.dir.join("response_stream.sse").display()
            )
        })?;
        // 将流式结果还原为便于阅读的“非流式”JSON
        let usage = result.usage.as_ref().map(|usage| {
            json!({
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
            })
        });
        let reconstructed = json!({
            "stream": false,
            "content": result.content,
            "reasoning": result.reasoning,
            "usage": usage,
            "tool_calls": result.tool_calls,
        });
        write_json(
            &self.dir.join("response_reconstructed.json"),
            &reconstructed,
        )?;
        Ok(())
    }

    /// 请求失败时写入错误响应正文。
    ///
    /// 参数:
    /// - `status`: 状态码
    /// - `body`: 错误正文
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn finish_error(&self, status: u16, body: &str) -> Result<()> {
        if !self.stream_buf.is_empty() {
            fs::write(self.dir.join("response_stream.sse"), &self.stream_buf).ok();
        }
        fs::write(
            self.dir.join("response_error.txt"),
            format!("status: {status}\n\n{body}\n"),
        )
        .with_context(|| {
            format!(
                "failed to write response error {}",
                self.dir.join("response_error.txt").display()
            )
        })?;
        Ok(())
    }

    /// 返回落盘目录（测试用）。
    #[cfg(test)]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// 读取当前线程绑定的会话 ID。
///
/// 返回:
/// - 会话 ID
fn current_session_id() -> Option<String> {
    CURRENT_SESSION_ID.with(|cell| cell.borrow().clone())
}

/// 判断环境变量是否为真值。
///
/// 参数:
/// - `value`: 原始环境变量
///
/// 返回:
/// - 是否开启
fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "debug"
    )
}

/// 脱敏头字段值。
///
/// 参数:
/// - `name`: 头名
/// - `value`: 原始值
///
/// 返回:
/// - 脱敏后的值
fn redact_header_value(name: &str, value: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower == "authorization" || lower == "x-api-key" || lower == "api-key" {
        if let Some(rest) = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
        {
            return format!("Bearer {}", redact_secret(rest));
        }
        return redact_secret(value);
    }
    value.to_string()
}

/// 脱敏密钥，仅保留前后少量字符。
///
/// 参数:
/// - `secret`: 密钥
///
/// 返回:
/// - 脱敏文本
fn redact_secret(secret: &str) -> String {
    let secret = secret.trim();
    if secret.len() <= 8 {
        return "***".to_string();
    }
    format!("{}…{}", &secret[..4], &secret[secret.len() - 4..])
}

/// 写入请求/响应头文本文件。
///
/// 参数:
/// - `path`: 目标路径
/// - `headers`: 头列表
///
/// 返回:
/// - 写入是否成功
fn write_headers_file(path: &Path, headers: &[(String, String)]) -> Result<()> {
    let mut lines = Vec::with_capacity(headers.len());
    for (name, value) in headers {
        lines.push(format!("{}: {}", name, redact_header_value(name, value)));
    }
    fs::write(path, lines.join("\n") + "\n")
        .with_context(|| format!("failed to write headers {}", path.display()))
}

/// 写入 pretty JSON 文件。
///
/// 参数:
/// - `path`: 目标路径
/// - `value`: JSON 值
///
/// 返回:
/// - 写入是否成功
fn write_json(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text + "\n").with_context(|| format!("failed to write {}", path.display()))
}

/// 构造 Bearer 请求头列表（已含脱敏用明文，写入时再脱敏）。
///
/// 参数:
/// - `api_key`: API Key
/// - `extra`: 额外头
///
/// 返回:
/// - 头列表
pub fn bearer_request_headers(api_key: &str, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut headers = vec![
        ("Authorization".to_string(), format!("Bearer {api_key}")),
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Accept".to_string(), "text/event-stream".to_string()),
    ];
    for (name, value) in extra {
        headers.push(((*name).to_string(), (*value).to_string()));
    }
    headers
}

/// 构造 Anthropic 请求头列表。
///
/// 参数:
/// - `api_key`: API Key
///
/// 返回:
/// - 头列表
pub fn anthropic_request_headers(api_key: &str) -> Vec<(String, String)> {
    vec![
        ("x-api-key".to_string(), api_key.to_string()),
        ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Accept".to_string(), "text/event-stream".to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::Usage;
    use tempfile::TempDir;

    fn test_paths(temp: &TempDir) -> SaiPaths {
        let root = temp.path().to_path_buf();
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish"),
            bash_hook_file: root.join("bash"),
            zsh_hook_file: root.join("zsh"),
            powershell_hook_file: root.join("ps1"),
        }
    }

    #[test]
    fn is_truthy_accepts_common_flags() {
        assert!(is_truthy("1"));
        assert!(is_truthy("true"));
        assert!(is_truthy("YES"));
        assert!(!is_truthy("0"));
        assert!(!is_truthy("false"));
    }

    #[test]
    fn redacts_authorization_bearer() {
        let value = redact_header_value("Authorization", "Bearer sk-abcdefghijklmnop");
        assert!(value.starts_with("Bearer sk-a"));
        assert!(value.contains('…'));
        assert!(!value.contains("efghijklmnop"));
    }

    #[test]
    fn records_request_and_reconstructed_response() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(&temp);
        let config = HttpDebugConfig {
            root: paths.cache_dir.join("debug-http"),
            session_filter: None,
        };
        let _guard = SessionGuard::new("sess-debug");
        let body = json!({"model": "gpt", "stream": true});
        let headers = bearer_request_headers("sk-test-key-12345678", &[]);
        let mut recorder = HttpDebugRecorder::start(
            &config,
            "POST",
            "https://example.com/v1/chat/completions",
            "openai",
            "openai-chat",
            &headers,
            &body,
        )
        .unwrap()
        .expect("recorder");
        recorder.append_stream_line(r#"data: {"choices":[{"delta":{"content":"你好"}}]}"#);
        recorder.append_stream_line("data: [DONE]");
        let result = ChatResult {
            content: "你好".to_string(),
            reasoning: None,
            usage: Some(Usage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            }),
            tool_calls: Vec::new(),
            duration_ms: 0,
        };
        recorder.finish_ok(&result).unwrap();

        let dir = recorder.dir();
        assert!(dir.join("request_body.json").is_file());
        assert!(dir.join("request_headers.txt").is_file());
        assert!(dir.join("response_stream.sse").is_file());
        let reconstructed = fs::read_to_string(dir.join("response_reconstructed.json")).unwrap();
        assert!(reconstructed.contains("你好"));
        assert!(reconstructed.contains("\"stream\": false"));
        let headers_text = fs::read_to_string(dir.join("request_headers.txt")).unwrap();
        assert!(!headers_text.contains("sk-test-key-12345678"));
    }

    #[test]
    fn session_filter_skips_other_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(&temp);
        let config = HttpDebugConfig {
            root: paths.cache_dir.join("debug-http"),
            session_filter: Some("only-this".to_string()),
        };
        let _guard = SessionGuard::new("other");
        let recorder = HttpDebugRecorder::start(
            &config,
            "POST",
            "https://example.com/v1/chat/completions",
            "openai",
            "openai-chat",
            &[],
            &json!({}),
        )
        .unwrap();
        assert!(recorder.is_none());
    }
}
