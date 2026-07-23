use super::{dynamic_tool_name, parse_sse_endpoint};

#[test]
fn dynamic_tool_name_is_stable_and_sanitized() {
    let name = dynamic_tool_name("File System", "read/file");
    assert!(name.starts_with("mcp_"));
    assert!(!name.contains('/'));
    assert!(!name.contains(' '));
}

#[test]
fn parse_sse_endpoint_absolute_and_relative() {
    let body = "event: endpoint\ndata: /messages?session=1\n\n";
    let url = parse_sse_endpoint(body, "http://127.0.0.1:3000/sse").unwrap();
    assert_eq!(url, "http://127.0.0.1:3000/messages?session=1");
    let body2 = "event: endpoint\ndata: http://example.com/m\n\n";
    assert_eq!(
        parse_sse_endpoint(body2, "http://127.0.0.1:3000/sse").unwrap(),
        "http://example.com/m"
    );
}
