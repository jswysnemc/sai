use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
};

/// 【服务商配置】【请求回归】目录请求使用选中多密钥，并保留返回的模型元数据。
/// 参数: 无；返回: 无
#[test]
fn catalog_request_uses_selected_key_and_metadata() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        while !request.windows(4).any(|part| part == b"\r\n\r\n") {
            let length = socket.read(&mut buffer).unwrap();
            assert!(length > 0);
            request.extend_from_slice(&buffer[..length]);
        }
        let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
        assert!(request.starts_with("get /v1/models "));
        assert!(request.contains("authorization: bearer fixture-b\r\n"));
        let body = r#"{"data":[{"id":"remote","context_window":128000,"max_output_tokens":8192}]}"#;
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let mut provider = ProviderConfig::default_openai();
    provider.base_url = format!("http://{address}/v1");
    provider.timeout_seconds = 3;
    provider.api_key = Some("single-unused".into());
    provider.api_keys =
        super::super::provider_forms::keys::parse_api_key_lines("fixture-a\nfixture-b", &[]);
    provider.api_key_selected = Some(provider.api_keys[1].id.clone());
    let result = fetch_models(&provider).unwrap();
    server.join().unwrap();
    assert_eq!(result.models, ["remote"]);
    assert_eq!(result.metadata["remote"].context_chars, Some(128000));
    assert_eq!(result.metadata["remote"].max_output_tokens, Some(8192));
    assert_eq!(provider.api_key.as_deref(), Some("single-unused"));
}
