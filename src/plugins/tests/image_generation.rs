use super::image_support::*;
use base64::Engine;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【图片生成测试】【完整响应】Base64 优先于 URL，首个非空密钥和 180 秒超时保持原行为。
#[tokio::test]
async fn generation_preserves_request_output_and_base64_precedence() {
    let bytes = b"fixture-image-bytes";
    let response = json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(bytes),"url":"https://unused.test/image"}]});
    let host = Arc::new(ImageHost::new(vec![(
        200,
        serde_json::to_vec(&response).unwrap(),
    )]));
    let plugin = runtime("image-generation", generation_settings(), host.clone());
    let result = plugin
        .call_tool(
            "generate_image",
            json!({"prompt":"  A Test_image  "}),
            context(),
        )
        .await
        .unwrap();
    let result: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(result["status"], "ok");
    assert_eq!(result["bytes"], bytes.len());
    assert_eq!(result["printed"], false);
    assert_eq!(result["print_error"], Value::Null);
    assert_eq!(result["path"], result["final_response_must_include_path"]);
    assert!(result["assistant_instruction"]
        .as_str()
        .unwrap()
        .contains("saved to disk"));
    let path = result["path"].as_str().unwrap();
    let name = std::path::Path::new(path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    assert!(name.starts_with("a-test-image-") && name.ends_with(".png"));
    assert_eq!(name.len(), "a-test-image-YYYYmmdd-HHMMSS.png".len());
    assert_eq!(host.writes.lock().unwrap()[0].1, bytes);
    assert!(host.downloads.lock().unwrap().is_empty());
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url,
        "https://api.example.test/v1/images/generations"
    );
    assert_eq!(requests[0].headers["authorization"], "Bearer fixture-key");
    assert_eq!(requests[0].timeout_ms, 180000);
    assert_eq!(
        serde_json::from_str::<Value>(requests[0].body.as_ref().unwrap()).unwrap(),
        json!({"model":"gpt-image-1","prompt":"A Test_image","n":1,"size":"auto"})
    );
}

/// 【图片生成测试】【URL 下载】API 返回 URL 时另发匿名 GET，保存下载字节而非 JSON 正文。
#[tokio::test]
async fn url_results_download_without_forwarding_api_credentials() {
    let host = Arc::new(ImageHost::new(vec![
        (
            200,
            br#"{"data":[{"url":"https://cdn.test/image?token=signed"}]}"#.to_vec(),
        ),
        (200, vec![1, 2, 3, 4]),
    ]));
    let plugin = runtime("image-generation", generation_settings(), host.clone());
    plugin
        .call_tool("generate_image", json!({"prompt":"中文"}), context())
        .await
        .unwrap();
    let downloads = host.downloads.lock().unwrap();
    assert_eq!(downloads.len(), 1);
    assert_eq!(downloads[0].timeout_ms, 180000);
    assert_eq!(downloads[0].url, "https://cdn.test/image?token=signed");
    assert!(downloads[0].headers.is_empty());
    assert_eq!(host.writes.lock().unwrap()[0].1, vec![1, 2, 3, 4]);
}

/// 【图片生成测试】【预览策略】关闭、不可用、成功和失败四种预览状态均保留已保存的图片。
#[tokio::test]
async fn preview_policy_uses_visible_tools_and_never_loses_the_saved_result() {
    for (auto, enabled, fail, printed, calls) in [
        (false, true, false, false, 0),
        (true, false, false, false, 0),
        (true, true, false, true, 1),
        (true, true, true, false, 1),
    ] {
        let host = Arc::new(ImageHost::new(vec![(
            200,
            br#"{"data":[{"b64_json":"aGVsbG8="}]}"#.to_vec(),
        )]));
        let mut settings = generation_settings();
        settings["auto_print"] = json!(auto);
        let services = Arc::new(PreviewServices {
            enabled,
            fail,
            ..Default::default()
        });
        let plugin = runtime("image-generation", settings, host.clone());
        let result = plugin
            .call_tool(
                "generate_image",
                json!({"prompt":"test"}),
                sai_plugin_runtime::InvocationContext {
                    services: Some(services.clone()),
                    ..context()
                },
            )
            .await
            .unwrap();
        let result: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["printed"], printed);
        assert_eq!(host.writes.lock().unwrap().len(), 1);
        assert_eq!(services.calls.lock().unwrap().len(), calls);
        if fail {
            assert!(result["print_error"]
                .as_str()
                .unwrap()
                .contains("fixture preview failed"));
        } else {
            assert!(result["print_error"].is_null());
        }
        if printed {
            assert!(result["assistant_instruction"]
                .as_str()
                .unwrap()
                .contains("already been printed"));
        }
    }
}

/// 【图片生成测试】【失败保存】接口错误、坏 JSON、空数组、无效 Base64 和下载失败都不写入文件。
#[tokio::test]
async fn failed_responses_never_publish_a_file() {
    for (responses, message) in [
        (
            vec![(400, b"bad request".to_vec())],
            "image API error (400): bad request",
        ),
        (vec![(200, b"broken".to_vec())], "invalid binary JSON"),
        (vec![(200, b"{}".to_vec())], "missing data array"),
        (vec![(200, br#"{"data":[]}"#.to_vec())], "data is empty"),
        (
            vec![(200, br#"{"data":[{}]}"#.to_vec())],
            "neither b64_json nor url",
        ),
        (
            vec![(
                200,
                br#"{"data":[{"b64_json":"%%%","url":"https://unused.test"}]}"#.to_vec(),
            )],
            "invalid base64",
        ),
        (
            vec![
                (200, br#"{"data":[{"url":"https://cdn.test"}]}"#.to_vec()),
                (404, vec![]),
            ],
            "failed to download generated image",
        ),
    ] {
        let host = Arc::new(ImageHost::new(responses));
        let plugin = runtime("image-generation", generation_settings(), host.clone());
        let error = plugin
            .call_tool("generate_image", json!({"prompt":"test"}), context())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(message), "{error:#}");
        assert!(host.writes.lock().unwrap().is_empty());
    }
}
