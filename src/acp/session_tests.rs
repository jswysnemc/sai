use super::client_identity::client_info_name;
use super::prompt::{blocks as prompt_blocks, split_data_url};
use crate::agent_engine::{AcpPromptContext, TurnRequest};

/// Codex ACP 要保留 Codex CLI 默认来源，其它 ACP 内核仍声明 Sai。
#[test]
fn codex_uses_non_originating_app_server_identity() {
    assert_eq!(client_info_name("codex"), "codex_app_server_daemon");
    assert_eq!(client_info_name("claude_code"), "sai");
    assert_eq!(client_info_name("custom"), "sai");
}

/// 文本与受支持的图片必须按输入顺序转换为 ACP 内容块。
#[test]
fn prompt_carries_text_and_images() {
    let request = TurnRequest {
        input: "看这张图".to_string(),
        image_urls: vec!["data:image/png;base64,AAAA".to_string()],
        cwd: std::path::PathBuf::from("/tmp"),
        contexts: Vec::new(),
    };
    let blocks = prompt_blocks(
        &request,
        &super::capabilities::AcpCapabilities {
            prompt_image: true,
            ..Default::default()
        },
    )
    .unwrap();
    let value = serde_json::to_value(blocks).unwrap();
    let array = value.as_array().unwrap();
    assert_eq!(array[0]["type"], "text");
    assert_eq!(array[0]["text"], "看这张图");
    assert_eq!(array[1]["type"], "image");
    assert_eq!(array[1]["mimeType"], "image/png");
    assert_eq!(array[1]["data"], "AAAA");
}

/// 非 base64 的 data URL 无法拆成 ACP 需要的字段，应当跳过坏数据。
#[test]
fn skips_unsupported_image_urls() {
    let request = TurnRequest {
        input: "问题".to_string(),
        image_urls: vec!["https://example.com/a.png".to_string()],
        cwd: std::path::PathBuf::from("/tmp"),
        contexts: Vec::new(),
    };
    assert_eq!(
        prompt_blocks(
            &request,
            &super::capabilities::AcpCapabilities {
                prompt_image: true,
                ..Default::default()
            },
        )
        .unwrap()
        .len(),
        1
    );
}

/// 支持嵌入上下文时，记忆与目标必须作为标准 ACP resource 发送。
#[test]
fn prompt_carries_sai_context_as_embedded_resources() {
    let request = TurnRequest {
        input: "继续".to_string(),
        image_urls: Vec::new(),
        cwd: std::path::PathBuf::from("/tmp"),
        contexts: vec![AcpPromptContext {
            uri: "sai://goal/active".to_string(),
            text: "<active-goal>ship</active-goal>".to_string(),
        }],
    };
    let blocks = prompt_blocks(
        &request,
        &super::capabilities::AcpCapabilities {
            embedded_context: true,
            ..Default::default()
        },
    )
    .unwrap();
    let value = serde_json::to_value(blocks).unwrap();

    assert_eq!(value[1]["type"], "resource");
    assert_eq!(value[1]["resource"]["uri"], "sai://goal/active");
    assert_eq!(
        value[1]["resource"]["text"],
        "<active-goal>ship</active-goal>"
    );
}

/// 未声明 embeddedContext 的 agent 仍可对话，但不会收到不支持的资源块。
#[test]
fn prompt_skips_sai_context_when_agent_lacks_embedded_context() {
    let request = TurnRequest {
        input: "继续".to_string(),
        image_urls: Vec::new(),
        cwd: std::path::PathBuf::from("/tmp"),
        contexts: vec![AcpPromptContext {
            uri: "sai://memory/association".to_string(),
            text: "memory".to_string(),
        }],
    };

    assert_eq!(
        prompt_blocks(&request, &super::capabilities::AcpCapabilities::default())
            .unwrap()
            .len(),
        1
    );
}

/// data URL 必须拆成媒体类型与 base64 内容。
#[test]
fn splits_base64_data_urls() {
    assert_eq!(
        split_data_url("data:image/jpeg;base64,QUJD"),
        Some(("image/jpeg".to_string(), "QUJD".to_string()))
    );
    assert!(split_data_url("data:text/plain,hello").is_none());
}
