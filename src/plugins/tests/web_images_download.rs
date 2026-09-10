use super::web_images_support::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{atomic::Ordering, Arc};

/// 【搜图测试】【保存结果】实际字节决定摘要文件名及尺寸，返回字段保持原工具契约。
/// @returns 无；结果不包含缓冲句柄
#[tokio::test]
async fn downloaded_images_use_content_hashes_and_actual_dimensions() {
    let bytes = png(640, 480, 1);
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let host = Arc::new(WebImageHost::candidates(
        vec![candidate(1)],
        vec![(200, bytes.clone())],
    ));
    let plugin = runtime(settings(), host.clone());
    let result: Value = serde_json::from_str(
        &plugin
            .call_tool(TOOL, json!({"query":"mountain","count":1}), context())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["result_role"], "downloaded_image_candidates");
    assert_eq!(result["vision_screening"], "unavailable");
    assert_eq!(result["count"], 1);
    let image = &result["images"][0];
    assert_eq!(image["mime_type"], "image/png");
    assert_eq!(image["width"], 640);
    assert_eq!(image["height"], 480);
    assert_eq!(image["size_bytes"], bytes.len());
    assert_eq!(image["sha256"], digest);
    assert_eq!(image["vision"]["status"], "not_requested");
    assert_eq!(image["used_thumbnail"], false);
    assert!(image.get("data").is_none());
    let name = std::path::Path::new(image["local_path"].as_str().unwrap())
        .file_name()
        .unwrap()
        .to_string_lossy();
    assert_eq!(name, format!("webimg-{digest}.png"));
    assert_eq!(host.images.writes.lock().unwrap()[0].1, bytes);
    let requests = host.images.downloads.lock().unwrap();
    assert_eq!(requests[0].max_bytes, 4 * 1024 * 1024);
    assert!(requests[0].headers.is_empty());
}

/// 【搜图测试】【缩略图回退】原图状态失败、空正文和未知类型均尝试缩略图，最终 URL 可提供媒体类型。
/// @returns 无；结果明确标记缩略图来源
#[tokio::test]
async fn bad_originals_use_thumbnails_and_final_redirect_urls() {
    for (status, original) in [
        (404, b"missing".to_vec()),
        (200, vec![]),
        (200, b"unknown".to_vec()),
    ] {
        let mut item = candidate(1);
        item["thumbnail"] = json!("https://thumbnail.test/image");
        let host = Arc::new(WebImageHost::candidates(
            vec![item],
            vec![(status, original), (200, b"extension-only".to_vec())],
        ));
        host.images.responses.lock().unwrap()[1].url = "https://redirect.test/image.gif".into();
        let plugin = runtime(settings(), host.clone());
        let result: Value = serde_json::from_str(
            &plugin
                .call_tool(TOOL, json!({"query":"mountain","count":1}), context())
                .await
                .unwrap(),
        )
        .unwrap();
        let image = &result["images"][0];
        assert_eq!(image["mime_type"], "image/gif");
        assert_eq!(image["used_thumbnail"], true);
        assert_eq!(image["image_url"], "https://image.test/1");
        assert_eq!(host.images.downloads.lock().unwrap().len(), 2);
    }
}

/// 【搜图测试】【摘要去重】不同地址的同一图片只占一个结果，继续下载后续候选补足数量。
/// @returns 无；最终结果具有不同内容摘要
#[tokio::test]
async fn duplicate_downloaded_content_does_not_consume_the_requested_count() {
    let first = png(640, 480, 1);
    let host = Arc::new(WebImageHost::candidates(
        (1..=3).map(candidate).collect(),
        vec![(200, first.clone()), (200, first), (200, png(640, 480, 2))],
    ));
    let plugin = runtime(settings(), host.clone());
    let result: Value = serde_json::from_str(
        &plugin
            .call_tool(TOOL, json!({"query":"mountain","count":2}), context())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["count"], 2);
    assert_ne!(result["images"][0]["sha256"], result["images"][1]["sha256"]);
    assert_eq!(result["images"][1]["image_url"], "https://image.test/3");
    assert_eq!(host.images.downloads.lock().unwrap().len(), 3);
}

/// 【搜图测试】【候选预算】下载持续失败时遵守原尝试上限，避免无限获取候选。
/// @returns 无；单张请求最多尝试七个候选
#[tokio::test]
async fn failed_downloads_stop_at_the_probe_limit() {
    let host = Arc::new(WebImageHost::candidates(
        (1..=30).map(candidate).collect(),
        vec![(404, vec![]); 30],
    ));
    let plugin = runtime(settings(), host.clone());
    let error = plugin
        .call_tool(TOOL, json!({"query":"mountain","count":1}), context())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("no image could be downloaded"));
    assert_eq!(host.images.downloads.lock().unwrap().len(), 7);
    assert!(host.images.writes.lock().unwrap().is_empty());
}

/// 【搜图测试】【磁盘失败】写入错误直接返回，不把未保存图片当作成功结果或继续替代下载。
/// @returns 无；原始错误仍可定位保存失败
#[tokio::test]
async fn write_failures_abort_instead_of_reporting_unsaved_images() {
    let host = Arc::new(WebImageHost::candidates(
        vec![candidate(1), candidate(2)],
        vec![(200, png(640, 480, 1))],
    ));
    host.fail_write.store(true, Ordering::SeqCst);
    let plugin = runtime(settings(), host.clone());
    let error = plugin
        .call_tool(TOOL, json!({"query":"mountain","count":1}), context())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("fixture disk write failed"));
    assert_eq!(host.images.downloads.lock().unwrap().len(), 1);
    assert!(host.images.writes.lock().unwrap().is_empty());
}
