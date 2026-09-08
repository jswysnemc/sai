use super::support::{call_json, runtime, FixtureHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

const TOOL: &str = "protondb_query";
const COUNTS: &str = r#"{"reports":1234567,"timestamp":1735689600}"#;
const SUMMARY: &str = r#"{"tier":"gold","confidence":"good","score":0.9,"total":27,"bestReportedTier":"platinum","trendingTier":"gold"}"#;

/// 【插件测试】【ProtonDB 查询】真实 Lua 使用 GET 搜索，保留游戏、评级和完整评论字段。
#[tokio::test]
async fn protondb_search_is_readonly_and_preserves_the_response_contract() {
    let search = json!({"hits":[{
        "objectID":"1245620", "name":"ELDEN RING", "oslist":["Windows",null,"Steam Deck Verified"],
    }]})
    .to_string();
    let reports = json!({"total":3, "reports":[{
        "contributor":{"steam":{"nickname":"测试用户","reportTally":5,"playtime":600}},
        "timestamp":1735689600,
        "responses":{
            "startsPlay":"yes","verdict":"no","verdictOob":"yes",
            "variant":"ge","customProtonVersion":"GE-Proton9-4","protonVersion":"ignored",
            "launchOptions":"  PROTON_LOG=1 %command%  ",
            "concludingNotes":"old note",
            "audioFaults":"yes","graphicalFaults":"yes","windowingFaults":"yes","inputFaults":"no",
            "followUp":{"audioFaults":{"desync":true,"crackling":false},"graphicalFaults":"minorArtifacts","windowingFaults":["ignored"]},
            "notes":{"concludingNotes":"  Works great!  ","graphicalFaults":" some flickering ","windowingFaults":" "},
        },
    }]}).to_string();
    let host = Arc::new(FixtureHost::new(&[
        (200, &search),
        (200, SUMMARY),
        (200, COUNTS),
        (200, &reports),
    ]));
    let plugin = runtime("protondb", host.clone());
    let result = call_json(&plugin, TOOL, json!({"query":"  Elden Ring + 中文  "})).await;
    assert_eq!(
        result,
        json!({
            "app_id":1245620,"game_name":"ELDEN RING","oslist":["Windows","Steam Deck Verified"],
            "summary":{"tier":"gold","confidence":"good","score":0.9,"total":27,"best_reported_tier":"platinum","trending_tier":"gold"},
            "reports":{"total":3,"returned":1,"items":[{
                "author":"测试用户","report_count":5,"playtime_hours":10,"date":"2025-01-01",
                "recommended":"recommended","proton_version":"GE-Proton9-4",
                "launch_options":"PROTON_LOG=1 %command%",
                "faults":[
                    {"type":"audio","details":["crackling","desync"],"note":null},
                    {"type":"graphics","details":["minorArtifacts"],"note":"some flickering"},
                    {"type":"windowing","details":[],"note":null},
                ],
                "notes":"Works great!",
            }]},
            "protondb_url":"https://www.protondb.com/app/1245620",
        })
    );
    let requests = host.requests.lock().unwrap();
    assert!(requests.iter().all(|request| request.timeout_ms == 20000));
    assert!(requests
        .iter()
        .all(|request| request.method == "GET" && request.body.is_none()));
    let url = reqwest::Url::parse(&requests[0].url).unwrap();
    assert_eq!(url.path(), "/1/indexes/steamdb");
    let query: BTreeMap<String, String> = url.query_pairs().into_owned().collect();
    assert_eq!(query["query"], "Elden Ring + 中文");
    assert_eq!(
        serde_json::from_str::<Value>(&query["facetFilters"]).unwrap(),
        json!([["appType:Game"]])
    );
    assert_eq!(
        serde_json::from_str::<Value>(&query["attributesToRetrieve"]).unwrap(),
        json!(["name", "objectID", "oslist"])
    );
    assert_eq!(query["hitsPerPage"], "1");
    assert_eq!(query["page"], "0");
    assert_eq!(
        requests[0].headers["x-algolia-application-id"],
        "94HE6YATEI"
    );
    assert_eq!(
        requests[1].url,
        "https://www.protondb.com/api/v1/reports/summaries/1245620.json"
    );
    assert_eq!(requests[2].url, "https://www.protondb.com/data/counts.json");
    assert_eq!(
        requests[3].url,
        "https://www.protondb.com/data/reports/all-devices/app/37028798.json"
    );
}

/// 【插件测试】【数字查询】名称查询失败或命中其他游戏时继续使用原 App ID。
#[tokio::test]
async fn protondb_numeric_queries_keep_the_id_when_name_lookup_fails() {
    for (status, search, name) in [
        (503, "unavailable", "1245620"),
        (
            200,
            r#"{"hits":[{"objectID":"7","name":"Wrong game"}]}"#,
            "1245620",
        ),
        (
            200,
            r#"{"hits":[{"objectID":"1245620","name":"ELDEN RING","oslist":[]}]}"#,
            "ELDEN RING",
        ),
    ] {
        let host = Arc::new(FixtureHost::new(&[
            (status, search),
            (200, "{}"),
            (200, COUNTS),
            (200, "{}"),
        ]));
        let plugin = runtime("protondb", host);
        let result = call_json(&plugin, TOOL, json!({"query":"1245620"})).await;
        assert_eq!(result["app_id"], 1245620);
        assert_eq!(result["game_name"], name);
        assert_eq!(result["oslist"], json!([]));
        assert_eq!(
            result["summary"],
            json!({
                "tier":null,"confidence":null,"score":null,"total":null,
                "best_reported_tier":null,"trending_tier":null,
            })
        );
        assert_eq!(
            result["reports"],
            json!({"total":0,"returned":0,"items":[]})
        );
    }
}

/// 【插件测试】【地址哈希】固定外部向量覆盖普通值、零、超过浮点精度的整数和回绕。
#[tokio::test]
async fn protondb_report_urls_match_integer_hash_vectors() {
    for (id, count, timestamp, expected) in [
        (1_i64, 2_i64, 3_i64, 1075359677),
        (1245620, 1234567, 1735689600, 37028798),
        (0, 1, 1, 1273715678),
        (9007199254740993, 1500000, 1735689600, 1957213195),
        (
            9223372036854775807,
            9223372036854775700,
            1735689600,
            1069431454,
        ),
    ] {
        let counts = json!({"reports":count, "timestamp":timestamp}).to_string();
        let host = Arc::new(FixtureHost::new(&[
            (200, r#"{"hits":[]}"#),
            (200, "{}"),
            (200, &counts),
            (200, "{}"),
        ]));
        let plugin = runtime("protondb", host.clone());
        let result = call_json(&plugin, TOOL, json!({"query":id.to_string()})).await;
        assert_eq!(result["app_id"], id);
        let requests = host.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[3].url,
            format!("https://www.protondb.com/data/reports/all-devices/app/{expected}.json")
        );
    }
}

/// 【插件测试】【评论限制】保留默认十条、上限四十条及显式零条的语义。
#[tokio::test]
async fn protondb_limits_returned_reports_without_losing_totals() {
    let reports = json!({"total":99, "reports":vec![json!({});45]}).to_string();
    for (args, expected) in [
        (json!({"query":"1245620"}), 10),
        (json!({"query":"1245620","max_reports":99}), 40),
        (json!({"query":"1245620","max_reports":-1}), 10),
        (json!({"query":"1245620","max_reports":0}), 0),
    ] {
        let host = Arc::new(FixtureHost::new(&[
            (200, r#"{"hits":[]}"#),
            (200, SUMMARY),
            (200, COUNTS),
            (200, &reports),
        ]));
        let plugin = runtime("protondb", host);
        let result = call_json(&plugin, TOOL, args).await;
        assert_eq!(result["reports"]["total"], 99);
        assert_eq!(result["reports"]["returned"], expected);
        assert_eq!(
            result["reports"]["items"].as_array().unwrap().len(),
            expected
        );
    }
}

/// 【插件测试】【评论判定】覆盖推荐优先级、Proton 版本、闰日、可选文本和空字段。
#[tokio::test]
async fn protondb_report_fields_handle_variants_dates_and_missing_values() {
    let reports = json!({"reports":[
        {},
        {"timestamp":1709164800,"responses":{"startsPlay":"yes","variant":"experimental"}},
        {"timestamp":86400,"responses":{"startsPlay":"yes","verdict":"yes","verdictOob":"no","variant":"notListed","protonVersion":"9.0","concludingNotes":" legacy "}},
        {"responses":{"startsPlay":"no","verdictOob":"yes","variant":"ge"}},
        {"contributor":{"steam":{"playtime":30}},"responses":{"startsPlay":"yes","verdict":"yes","protonVersion":"8.0","launchOptions":" ","notes":{"concludingNotes":""},"concludingNotes":"ignored"}},
    ]}).to_string();
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"{"hits":[]}"#),
        (200, SUMMARY),
        (200, COUNTS),
        (200, &reports),
    ]));
    let plugin = runtime("protondb", host);
    let result = call_json(&plugin, TOOL, json!({"query":"1245620"})).await;
    let items = result["reports"]["items"].as_array().unwrap();
    assert_eq!(
        items[0],
        json!({
            "author":"anonymous","report_count":0,"playtime_hours":null,"date":"unknown",
            "recommended":"broken","proton_version":null,"launch_options":null,"faults":[],"notes":null,
        })
    );
    assert_eq!(items[1]["date"], "2024-02-29");
    assert_eq!(items[1]["recommended"], "unknown");
    assert_eq!(items[1]["proton_version"], "Proton Experimental");
    assert_eq!(items[2]["date"], "1970-01-02");
    assert_eq!(items[2]["recommended"], "not_recommended");
    assert_eq!(items[2]["proton_version"], "9.0");
    assert_eq!(items[2]["notes"], "legacy");
    assert_eq!(items[3]["recommended"], "broken");
    assert_eq!(items[3]["proton_version"], Value::Null);
    assert_eq!(items[4]["recommended"], "recommended");
    assert_eq!(items[4]["playtime_hours"], 0);
    assert_eq!(items[4]["launch_options"], Value::Null);
    assert_eq!(items[4]["notes"], Value::Null);
}

/// 【插件测试】【评论回退】计数、评论或网络失败不丢失已经取得的评级。
#[tokio::test]
async fn protondb_unavailable_reports_do_not_hide_the_summary() {
    for responses in [
        vec![(200, r#"{"hits":[]}"#), (200, SUMMARY), (200, "{}")],
        vec![
            (200, r#"{"hits":[]}"#),
            (200, SUMMARY),
            (200, COUNTS),
            (404, "missing"),
        ],
        vec![(200, r#"{"hits":[]}"#), (200, SUMMARY)],
    ] {
        let plugin = runtime("protondb", Arc::new(FixtureHost::new(&responses)));
        let result = call_json(&plugin, TOOL, json!({"query":"1245620"})).await;
        assert_eq!(result["summary"]["tier"], "gold");
        assert_eq!(result["summary"]["total"], 27);
        assert_eq!(
            result["reports"],
            json!({"total":0,"returned":0,"items":[]})
        );
    }
}

/// 【插件测试】【查询错误】空参数、非法标识和无命中结果明确失败，不继续获取评级。
#[tokio::test]
async fn protondb_rejects_invalid_ids_and_missing_games() {
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"{"hits":[]}"#),
        (200, r#"{"hits":[{"objectID":"12.5"}]}"#),
    ]));
    let plugin = runtime("protondb", host.clone());
    for query in [" ", "9223372036854775808"] {
        assert!(plugin
            .call_tool(TOOL, json!({"query":query}), InvocationContext::default())
            .await
            .is_err());
    }
    assert!(host.requests.lock().unwrap().is_empty());
    for (query, message) in [
        ("missing game", "no search results"),
        ("bad game", "invalid objectID"),
    ] {
        let error = plugin
            .call_tool(TOOL, json!({"query":query}), InvocationContext::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(message));
    }
    assert_eq!(host.requests.lock().unwrap().len(), 2);
}

/// 【插件测试】【评级错误】评级获取失败携带 App ID，避免返回不完整的成功结果。
#[tokio::test]
async fn protondb_summary_errors_stop_before_report_requests() {
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"{"hits":[]}"#),
        (503, "unavailable"),
    ]));
    let plugin = runtime("protondb", host.clone());
    let error = plugin
        .call_tool(
            TOOL,
            json!({"query":"1245620"}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("summary fetch failed for app 1245620"));
    assert!(message.contains("HTTP 503"));
    assert_eq!(host.requests.lock().unwrap().len(), 2);
}
