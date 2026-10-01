use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// 【模型接口】【传输测试】持有本地响应服务，释放时中止未完成连接。
struct Fixture {
    client: OpenAiCompatibleClient,
    server: tokio::task::JoinHandle<()>,
    _directory: tempfile::TempDir,
}

impl Drop for Fixture {
    /// 【模型接口】【传输测试】释放服务任务；无参数或返回值。
    fn drop(&mut self) {
        self.server.abort();
    }
}

/// 【模型接口】【传输测试】按延迟发送响应片段；返回配置了读取时限的真实客户端。
/// @param protocol 为接口协议；header_delay 为响应头延迟；chunks 为正文片段及发送前延迟
/// @returns 服务任务和隔离客户端
async fn fixture(
    protocol: &str,
    header_delay: Duration,
    chunks: Vec<(Duration, String)>,
) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut bytes = [0u8; 4096];
        // 1. 消费完整请求，避免关闭连接时未读正文导致 TCP 重置
        loop {
            let read = socket.read(&mut bytes).await.unwrap();
            if read == 0 {
                return;
            }
            request.extend_from_slice(&bytes[..read]);
            if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        // 2. 独立控制响应头和每个正文片段，验证首包与持续读取
        tokio::time::sleep(header_delay).await;
        let size: usize = chunks.iter().map(|(_, body)| body.len()).sum();
        let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n");
        if socket.write_all(headers.as_bytes()).await.is_err() {
            return;
        }
        for (delay, body) in chunks {
            tokio::time::sleep(delay).await;
            if socket.write_all(body.as_bytes()).await.is_err() {
                return;
            }
        }
    });
    let directory = tempfile::tempdir().unwrap();
    let mut provider = ProviderConfig::default_openai();
    provider.api_key = Some("test-key".into());
    provider.base_url = format!("http://{address}/v1");
    provider.protocol = protocol.into();
    provider.client_style = "default".into();
    provider.timeout_seconds = 1;
    let client = OpenAiCompatibleClient::new(
        &provider,
        &AppConfig::default(),
        &SaiPaths::for_tests(directory.path()),
    )
    .unwrap();
    Fixture {
        client,
        server,
        _directory: directory,
    }
}

/// 【模型接口】【传输测试】返回对应协议的正文增量；参数为协议，返回 SSE 文本。
fn partial(protocol: &str) -> String {
    let event = match protocol {
        "anthropic" => {
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"partial"}})
        }
        "openai-responses" => json!({"type":"response.output_text.delta","delta":"partial"}),
        _ => json!({"choices":[{"delta":{"content":"partial"}}]}),
    };
    format!("data: {event}\n\n")
}

/// 【模型接口】【传输测试】返回对应协议的结束事件；参数为协议，返回无末尾换行的 SSE 文本。
fn completed(protocol: &str) -> String {
    match protocol {
        "anthropic" => "data: {\"type\":\"message_stop\"}".into(),
        "openai-responses" => "data: {\"type\":\"response.completed\"}".into(),
        _ => "data: [DONE]".into(),
    }
}

/// 【模型接口】【完成校验】缺少结束事件必须报错，并保留已经交付的文本；无参数或返回值。
#[tokio::test]
async fn rejects_eof_without_completion_and_preserves_deltas() {
    for protocol in ["anthropic", "openai-responses", "openai-chat"] {
        let fixture = fixture(
            protocol,
            Duration::ZERO,
            vec![(Duration::ZERO, partial(protocol))],
        )
        .await;
        let mut delivered = String::new();
        let result = fixture
            .client
            .chat_stream(vec![ChatMessage::plain("user", "test")], vec![], |chunk| {
                delivered.push_str(&chunk.text);
                Ok(())
            })
            .await;
        assert!(result.is_err(), "{protocol} accepted an unfinished stream");
        assert_eq!(delivered, "partial", "{protocol}");
    }
}

/// 【模型接口】【完成校验】最后一条事件没有换行时仍正确结束；无参数或返回值。
#[tokio::test]
async fn accepts_terminal_event_without_trailing_newline() {
    for protocol in ["anthropic", "openai-responses", "openai-chat"] {
        let body = partial(protocol) + &completed(protocol);
        let fixture = fixture(protocol, Duration::ZERO, vec![(Duration::ZERO, body)]).await;
        let result = fixture
            .client
            .chat_stream(vec![ChatMessage::plain("user", "test")], vec![], |_| Ok(()))
            .await;
        assert_eq!(result.unwrap().content, "partial", "{protocol}");
    }
}

/// 【模型接口】【完成校验】Responses 明确声明 incomplete 时不能标记成功；无参数或返回值。
#[tokio::test]
async fn rejects_explicit_incomplete_response() {
    let body = partial("openai-responses") + "data: {\"type\":\"response.incomplete\",\"response\":{\"incomplete_details\":{\"reason\":\"max_output_tokens\"}}}\n\n";
    let fixture = fixture(
        "openai-responses",
        Duration::ZERO,
        vec![(Duration::ZERO, body)],
    )
    .await;
    let result = fixture
        .client
        .chat_stream(vec![ChatMessage::plain("user", "test")], vec![], |_| Ok(()))
        .await;
    assert!(result.is_err());
}

/// 【模型接口】【读取超时】响应头或正文不再到达时结束等待；无参数或返回值。
#[tokio::test]
async fn times_out_waiting_for_headers_or_body() {
    for protocol in ["openai-chat", "anthropic", "openai-responses"] {
        for headers_stall in [false, true] {
            let delay = Duration::from_secs(3);
            let fixture = fixture(
                protocol,
                if headers_stall { delay } else { Duration::ZERO },
                vec![(
                    if headers_stall { Duration::ZERO } else { delay },
                    partial(protocol),
                )],
            )
            .await;
            let result = tokio::time::timeout(
                Duration::from_millis(1800),
                fixture.client.chat_stream(
                    vec![ChatMessage::plain("user", "test")],
                    vec![],
                    |_| Ok(()),
                ),
            )
            .await;
            assert!(
                result.is_ok(),
                "{protocol} ignored the read timeout; headers={headers_stall}"
            );
            assert!(result.unwrap().is_err());
        }
    }
}

/// 【模型接口】【读取超时】持续输出可以超过单次读取时限；无参数或返回值。
#[tokio::test]
async fn ongoing_stream_can_outlive_read_timeout() {
    let protocol = "openai-chat";
    let mut chunks = vec![(Duration::from_millis(350), partial(protocol)); 4];
    chunks.push((Duration::ZERO, completed(protocol)));
    let fixture = fixture(protocol, Duration::ZERO, chunks).await;
    let result = fixture
        .client
        .chat_stream(vec![ChatMessage::plain("user", "test")], vec![], |_| Ok(()))
        .await;
    assert_eq!(result.unwrap().content, "partial".repeat(4));
}
