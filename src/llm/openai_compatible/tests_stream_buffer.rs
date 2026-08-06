/// 验证 UTF-8 多字节字符被网络分片切断时，行缓冲会跨数据块拼回完整字符。
#[test]
fn sse_buffer_preserves_utf8_split_across_byte_chunks() {
    let line = r#"data: {"choices":[{"delta":{"content":"等","tool_calls":null}}]}"#;
    // 1. 在「等」的中间字节处切开 UTF-8 三字节字符
    let split = line.find("等").unwrap() + 1;
    let mut buffer = Utf8LineBuffer::default();

    assert!(buffer.push(&line.as_bytes()[..split]).unwrap().is_empty());
    let lines = buffer.push(&line.as_bytes()[split..]).unwrap();

    assert!(lines.is_empty());
    assert_eq!(buffer.finish().unwrap(), vec![line]);
}

/// 验证旧的有损分块解码会破坏被切断的 UTF-8 字符。
#[test]
fn previous_lossy_chunk_decode_corrupts_split_utf8() {
    let text = "等";
    let mut decoded = String::new();

    decoded.push_str(&String::from_utf8_lossy(&text.as_bytes()[..1]));
    decoded.push_str(&String::from_utf8_lossy(&text.as_bytes()[1..]));

    assert_eq!(decoded, "\u{FFFD}\u{FFFD}\u{FFFD}");
}

/// 验证 Anthropic 风格数据聚合使用字节缓冲保留完整 UTF-8 字符。
#[test]
fn sse_data_buffer_preserves_utf8_across_chunks() {
    // 1. 使用双换行闭合 SSE 事件
    let payload = "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"等\"}}\n\n";
    let split = payload.find("等").unwrap() + 1;
    let mut buffer = SseDataBuffer::default();
    assert!(buffer
        .push(&payload.as_bytes()[..split])
        .unwrap()
        .is_empty());
    let events = buffer.push(&payload.as_bytes()[split..]).unwrap();
    assert_eq!(events.len(), 1);
    assert!(events[0].contains("等"));
}
