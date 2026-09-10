use super::{image_support::PreviewServices, web_images_support::*};
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, InvocationContext};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct ImageServices {
    preview: PreviewServices,
    disabled: bool,
    invalid_info: bool,
    replies: Mutex<VecDeque<Result<String, String>>>,
    requests: Mutex<Vec<(VisionRequest, Vec<u8>)>>,
}

#[async_trait]
impl InvocationServices for ImageServices {
    /// 【搜图测试】【视觉配置】模拟关闭、配置错误和独立可用三种状态。
    /// @returns 可信公开标识或配置错误
    fn vision_info(&self) -> Result<Option<VisionModelInfo>> {
        if self.invalid_info {
            bail!("fixture vision configuration failed");
        }
        Ok((!self.disabled).then(|| VisionModelInfo {
            provider_id: "vision-fixture".into(),
            model: "vision-model".into(),
        }))
    }

    /// 【搜图测试】【视觉响应】保存实际图片内容并消费固定筛选结果。
    /// @param request 提示词；image 为图片；max_bytes 为输出限制
    /// @returns 模型正文或固定失败
    async fn analyze_image(
        &self,
        request: VisionRequest,
        image: BinaryData,
        _: usize,
    ) -> Result<VisionResponse> {
        self.requests
            .lock()
            .unwrap()
            .push((request, image.bytes().to_vec()));
        let content = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("missing vision fixture")
            .map_err(anyhow::Error::msg)?;
        Ok(VisionResponse {
            content,
            provider_id: "vision-fixture".into(),
            model: "vision-model".into(),
        })
    }

    /// 【搜图测试】【显示目录】复用独立显示工具的可见性样本。
    /// @returns 当前可见工具
    fn tools(&self) -> Result<Vec<HostTool>> {
        self.preview.tools()
    }

    /// 【搜图测试】【显示执行】通过正式插件工具调用入口记录预览参数。
    /// @param name 工具名；arguments 为 JSON 参数
    /// @returns 显示结果或固定错误
    async fn call_tool(&self, name: &str, arguments: &str) -> Result<String> {
        self.preview.call_tool(name, arguments).await
    }

    /// 【搜图测试】【文本隔离】搜图筛选不能调用当前 Agent 的文本模型。
    /// @param request 文本请求；max_bytes 为上限
    /// @returns 明确错误
    async fn complete(&self, _: ModelRequest, _: usize) -> Result<ModelResponse> {
        bail!("unexpected text model request")
    }
}

/// 【搜图测试】【拒绝续选】视觉拒绝后继续候选，同内容图片不重复审核，接受结果保留模型描述。
/// @returns 无；视觉判断基于实际下载字节
#[tokio::test]
async fn rejected_images_continue_to_new_candidates_without_duplicate_reviews() {
    let first = png(640, 480, 1);
    let last = png(640, 480, 2);
    let host = Arc::new(WebImageHost::candidates(
        (1..=3).map(candidate).collect(),
        vec![
            (200, first.clone()),
            (200, first.clone()),
            (200, last.clone()),
        ],
    ));
    let mut config = settings();
    config["vision_screening_enabled"] = json!(true);
    let plugin = runtime(config, host.clone());
    let services = Arc::new(ImageServices {
        replies: Mutex::new(
            vec![
                Ok(r#"{"accepted":false,"description":"城市","reason":"不是山脉"}"#.into()),
                Ok(r#"{"accepted":true,"description":"山脉","reason":"匹配查询"}"#.into()),
            ]
            .into(),
        ),
        ..Default::default()
    });
    let result: Value = serde_json::from_str(
        &plugin
            .call_tool(
                TOOL,
                json!({"query":"mountain","count":1}),
                InvocationContext {
                    services: Some(services.clone()),
                    ..context()
                },
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["count"], 1);
    assert_eq!(result["rejected_by_vision"], 1);
    assert_eq!(result["vision_screening"], "enabled");
    let image = &result["images"][0];
    assert_eq!(image["image_url"], "https://image.test/3");
    assert_eq!(image["vision"]["description"], "山脉");
    assert_eq!(image["vision"]["provider_id"], "vision-fixture");
    assert_eq!(image["vision"]["model"], "vision-model");
    let requests = services.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].1, first);
    assert_eq!(requests[1].1, last);
    assert!(requests[1].0.prompt.contains("mountain"));
    assert!(requests[1].0.prompt.contains("https://page.test/3"));
}

/// 【搜图测试】【失败保留】关闭、不可用、配置错误与模型失败都保留已下载图片，并准确标记状态。
/// @returns 无；失败不能伪装为审核通过
#[tokio::test]
async fn unavailable_or_failed_screening_keeps_images_with_explicit_status() {
    for (enabled, disabled, invalid_info, expected, calls) in [
        (false, false, false, "not_requested", 0),
        (true, true, false, "not_requested", 0),
        (true, false, true, "failed", 0),
        (true, false, false, "failed", 1),
    ] {
        let host = Arc::new(WebImageHost::candidates(
            vec![candidate(1)],
            vec![(200, png(640, 480, 1))],
        ));
        let mut config = settings();
        config["vision_screening_enabled"] = json!(enabled);
        let plugin = runtime(config, host.clone());
        let services = Arc::new(ImageServices {
            disabled,
            invalid_info,
            replies: Mutex::new(vec![Err("fixture vision request failed".into())].into()),
            ..Default::default()
        });
        let result: Value = serde_json::from_str(
            &plugin
                .call_tool(
                    TOOL,
                    json!({"query":"mountain","count":1}),
                    InvocationContext {
                        services: Some(services.clone()),
                        ..context()
                    },
                )
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["images"][0]["vision"]["status"], expected);
        assert_eq!(result["images"][0]["vision"]["accepted"], true);
        assert_eq!(services.requests.lock().unwrap().len(), calls);
        assert_eq!(host.images.writes.lock().unwrap().len(), 1);
        if expected == "failed" {
            assert!(result["images"][0]["vision"]["error"]
                .as_str()
                .unwrap()
                .contains("fixture vision"));
        }
    }
}

/// 【搜图测试】【视觉图片上限】允许下载较大图片时，超过十 MiB 的图片保留但不发送视觉请求。
/// @returns 无；报告明确说明跳过的原因
#[tokio::test]
async fn large_downloads_are_saved_without_exceeding_the_vision_image_limit() {
    let mut bytes = png(640, 480, 1);
    bytes.resize(10 * 1024 * 1024 + 1, 0);
    let host = Arc::new(WebImageHost::candidates(
        vec![candidate(1)],
        vec![(200, bytes)],
    ));
    let mut config = settings();
    config["vision_screening_enabled"] = json!(true);
    config["max_download_mb"] = json!(16);
    let plugin = runtime(config, host);
    let services = Arc::new(ImageServices::default());
    let result: Value = serde_json::from_str(
        &plugin
            .call_tool(
                TOOL,
                json!({"query":"mountain","count":1}),
                InvocationContext {
                    services: Some(services.clone()),
                    ..context()
                },
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["count"], 1);
    assert_eq!(result["images"][0]["vision"]["status"], "failed");
    assert!(result["images"][0]["vision"]["error"]
        .as_str()
        .unwrap()
        .contains("image too large"));
    assert!(services.requests.lock().unwrap().is_empty());
}

/// 【搜图测试】【独立预览】显示开关、数量和失败互不影响下载结果，最多调用显示工具五次。
/// @returns 无；输出标记仅在实际尝试预览时出现
#[tokio::test]
async fn previews_follow_visible_tool_and_count_limits_without_losing_results() {
    for (requested, visible, fail, count, expected_calls) in [
        (false, true, false, 9, 0),
        (true, false, false, 9, 0),
        (true, true, false, 0, 0),
        (true, true, false, 9, 5),
        (true, true, true, 2, 2),
    ] {
        let host = Arc::new(WebImageHost::candidates(
            (1..=6).map(candidate).collect(),
            (1..=6).map(|index| (200, png(640, 480, index))).collect(),
        ));
        let mut config = settings();
        config["auto_preview"] = json!(true);
        config["max_results"] = json!(10);
        let plugin = runtime(config, host.clone());
        let services = Arc::new(ImageServices {
            preview: PreviewServices {
                enabled: visible,
                fail,
                ..Default::default()
            },
            ..Default::default()
        });
        let progress = Arc::new(Mutex::new(Vec::new()));
        let captured = progress.clone();
        let result: Value = serde_json::from_str(
            &plugin
                .call_tool(
                    TOOL,
                    json!({"query":"mountain","count":6,"preview":requested,"preview_count":count}),
                    InvocationContext {
                        services: Some(services.clone()),
                        progress: Some(Arc::new(move |text| captured.lock().unwrap().push(text))),
                        ..context()
                    },
                )
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["count"], 6);
        assert_eq!(result["printed"], expected_calls > 0 && !fail);
        assert_eq!(
            result["print_errors"].as_array().unwrap().len(),
            if fail { expected_calls } else { 0 }
        );
        let calls = services.preview.calls.lock().unwrap();
        assert_eq!(calls.len(), expected_calls);
        for (index, call) in calls.iter().enumerate() {
            assert_eq!(call["image"], result["images"][index]["local_path"]);
        }
        assert_eq!(
            progress
                .lock()
                .unwrap()
                .iter()
                .filter(|text| *text == "__external_output__")
                .count(),
            usize::from(expected_calls > 0)
        );
        assert_eq!(host.images.writes.lock().unwrap().len(), 6);
    }
}
