use super::web_images_support::*;
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【搜图测试】【只读查询】实际入口保留搜索参数、语言头和顺序，只读调用不下载、不请求视觉模型。
/// @returns 无；元数据结果保留原公开字段
#[tokio::test]
async fn readonly_queries_preserve_search_requests_without_downloading() {
    let host = Arc::new(WebImageHost::candidates(
        vec![candidate(1), candidate(2)],
        vec![],
    ));
    let mut config = settings();
    config["auto_preview"] = json!(true);
    config["vision_screening_enabled"] = json!(true);
    let plugin = runtime(config, host.clone());
    let result: Value = serde_json::from_str(
        &plugin
            .call_tool(
                TOOL,
                json!({"query":"  山脉 *~ ","count":1,"safe_search":false}),
                InvocationContext::default(),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["mode"], "metadata_only");
    assert_eq!(result["query"], "山脉 *~");
    assert_eq!(result["count"], 1);
    assert_eq!(result["images"][0]["source"], "DuckDuckGo Images");
    assert!(result["images"][0].get("local_path").is_none());
    assert!(host.images.downloads.lock().unwrap().is_empty());
    assert!(host.images.writes.lock().unwrap().is_empty());
    let requests = host.search.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].url,
        "https://duckduckgo.com/?q=%E5%B1%B1%E8%84%89+*%7E&iax=images&ia=images"
    );
    let query = reqwest::Url::parse(&requests[1].url)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(query["vqd"], "1-2-3");
    assert_eq!(query["p"], "-1");
    assert_eq!(query["q"], "山脉 *~");
    assert_eq!(
        requests[1].headers["referer"],
        "https://duckduckgo.com/?q=%E5%B1%B1%E8%84%89%20%2A~&iax=images&ia=images"
    );
    assert_eq!(
        requests[0].headers["accept-language"],
        "zh-CN,zh;q=0.9,en;q=0.8"
    );
    assert_eq!(requests[0].timeout_ms, 20_000);
}

/// 【搜图测试】【失败回退】HTTP、令牌、JSON 和空结果失败时都使用 Bing，保留安全搜索参数。
/// @returns 无；第二来源结果通过同一公开工具返回
#[tokio::test]
async fn search_failures_fall_back_to_bing() {
    for initial in [
        vec![(503, "unavailable")],
        vec![(200, "no token")],
        vec![(200, "vqd='123'"), (200, "invalid JSON")],
        vec![(200, "vqd='123'"), (200, "{\"results\":[]}")],
    ] {
        let html = bing(&[candidate(2), candidate(3)]);
        let mut responses = initial;
        responses.push((200, &html));
        let host = Arc::new(WebImageHost::new(&responses, vec![]));
        let plugin = runtime(settings(), host.clone());
        let result: Value = serde_json::from_str(
            &plugin
                .call_tool(
                    TOOL,
                    json!({"query":"mountain","count":2}),
                    InvocationContext::default(),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["count"], 2);
        assert_eq!(result["images"][0]["source"], "Bing Images");
        let requests = host.search.requests.lock().unwrap();
        assert_eq!(
            requests.last().unwrap().url,
            "https://www.bing.com/images/search?q=mountain&first=1&safeSearch=Strict"
        );
    }
}

/// 【搜图测试】【不足补齐】两个来源合并后按地址去重并排序，数量遵守用户请求及配置上限。
/// @returns 无；高相关候选优先且重复地址不占据结果名额
#[tokio::test]
async fn insufficient_primary_results_are_merged_ranked_and_limited() {
    let mut primary = candidate(1);
    primary["title"] = json!("unrelated");
    let mut duplicate = primary.clone();
    duplicate["image"] = json!("https://IMAGE.test/1?token=other");
    let html = bing(&[duplicate, candidate(2), candidate(3), candidate(4)]);
    let body = json!({"results":[primary]}).to_string();
    let host = Arc::new(WebImageHost::new(
        &[(200, "vqd='123'"), (200, &body), (200, &html)],
        vec![],
    ));
    let mut config = settings();
    config["max_results"] = json!(3);
    let plugin = runtime(config, host.clone());
    let result: Value = serde_json::from_str(
        &plugin
            .call_tool(
                TOOL,
                json!({"query":"mountain","count":99}),
                InvocationContext::default(),
            )
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["count"], 3);
    assert_eq!(
        result["images"]
            .as_array()
            .unwrap()
            .iter()
            .map(|image| image["image_url"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "https://image.test/2",
            "https://image.test/3",
            "https://image.test/4"
        ]
    );
    assert_eq!(host.search.requests.lock().unwrap().len(), 3);
}

/// 【搜图测试】【输入边界】缺少数量、错误类型、空查询和未知参数在网络请求前失败。
/// @returns 无；非法输入不消费测试响应
#[tokio::test]
async fn invalid_queries_never_start_search_requests() {
    let host = Arc::new(WebImageHost::default());
    let plugin = runtime(settings(), host.clone());
    for arguments in [
        json!({"query":"mountain"}),
        json!({"query":"mountain","count":-1}),
        json!({"query":"mountain","count":1.5}),
        json!({"query":"mountain","count":"1"}),
        json!({"query":"  ","count":1}),
        json!({"query":"mountain","count":1,"unknown":true}),
    ] {
        assert!(plugin
            .call_tool(TOOL, arguments, InvocationContext::default())
            .await
            .is_err());
    }
    assert!(host.search.requests.lock().unwrap().is_empty());
}

/// 【搜图测试】【无结果】两个引擎均未提供有效候选时明确失败，不返回伪造的成功结果。
/// @returns 无；错误保留原消息语义
#[tokio::test]
async fn both_search_sources_empty_is_an_explicit_error() {
    let plugin = runtime(
        settings(),
        Arc::new(WebImageHost::new(
            &[(500, "failed"), (200, "empty")],
            vec![],
        )),
    );
    let error = plugin
        .call_tool(
            TOOL,
            json!({"query":"mountain","count":1}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("image search returned no results"));
}
