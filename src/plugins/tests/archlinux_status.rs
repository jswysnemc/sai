use super::support::{call_json, runtime, FixtureHost};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【插件测试】【Arch 状态】保留未结束事件、受影响服务、详情状态与最新停机。
#[tokio::test]
async fn arch_status_combines_aur_incidents_and_downtime() {
    let events = json!({"results":[{
        "title":"Network incident", "type":"incident", "eventType":"down",
        "timestamp":1735689600, "timeGMT":"raw-time", "endDateGMT":null,
        "content":"Investigating\nAffected services:\n\n- aur.archlinux.org\n- - mirror\n\nUpdates below",
        "status":"investigating",
    }]}).to_string();
    let monitors = json!({"data":[
        {"name":"Other", "monitorId":1, "statusClass":"success"},
        {"name":"Package service", "url":"https://AUR.archlinux.org", "monitorId":42, "statusClass":"success"},
    ]}).to_string();
    let detail = json!({"monitor":{"statusClass":"Danger","logs":[
        {"label":"Up","dateGMTISO":"now"},
        {"label":"Down","dateGMTISO":"2025-01-01T00:00:00Z","duration":"3m","reason":{"detail":{"short":"HTTP 503"}}},
    ]}}).to_string();
    let host = Arc::new(FixtureHost::new(&[
        (200, &events),
        (200, &monitors),
        (200, &detail),
    ]));
    let plugin = runtime("archlinux", host.clone());
    let result = call_json(&plugin, "aur_check_status", json!({})).await;
    assert_eq!(
        result,
        json!({
            "success":true, "current_state":"down", "is_degraded":true,
            "degraded_reason":"Arch status page has an unresolved incident affecting AUR",
            "latest_down":{"started_at":"2025-01-01T00:00:00Z","duration":"3m","reason":"HTTP 503"},
            "latest_event":{
                "title":"Network incident","type":"incident","event_type":"down","is_active":true,
                "started_at":"2025-01-01T00:00:00+00:00","started_at_raw":"raw-time","ended_at":null,
                "content":"Investigating\nAffected services:\n\n- aur.archlinux.org\n- - mirror\n\nUpdates below",
                "status":"investigating","affected_services":["aur.archlinux.org","mirror"],
            },
            "monitor":{"name":"Package service","status_class":"Danger","monitor_id":42},
            "source":"https://status.archlinux.org",
        })
    );
    let requests = host.requests.lock().unwrap();
    assert_eq!(
        requests[0].url,
        "https://status.archlinux.org/api/getEventFeed/vmM5ruWEAB"
    );
    assert_eq!(
        requests[1].url,
        "https://status.archlinux.org/api/getMonitorList/vmM5ruWEAB"
    );
    assert_eq!(
        requests[2].url,
        "https://status.archlinux.org/api/getMonitor/vmM5ruWEAB?m=42"
    );
    assert!(requests.iter().all(|request| request.method == "GET"));
    assert!(requests.iter().all(|request| request.timeout_ms == 10000));
}

/// 【插件测试】【详情回退】详情不可用时保留列表状态；已结束事件不继续标记为降级。
#[tokio::test]
async fn arch_status_falls_back_to_the_list_for_unavailable_details() {
    let events = json!({"results":[{
        "title":"AUR incident","description":"Affected services:\n- AUR","endDate":"resolved"
    }]})
    .to_string();
    let monitors = r#"{"data":[{"name":"AUR","monitorId":7,"statusClass":"success"}]}"#;
    for (status, body) in [(404, "missing"), (200, "not json")] {
        let host = Arc::new(FixtureHost::new(&[
            (200, &events),
            (200, monitors),
            (status, body),
        ]));
        let plugin = runtime("archlinux", host);
        let result = call_json(&plugin, "aur_check_status", json!({})).await;
        assert_eq!(result["current_state"], "up");
        assert_eq!(result["is_degraded"], false);
        assert_eq!(result["degraded_reason"], Value::Null);
        assert_eq!(result["latest_event"]["is_active"], false);
        assert_eq!(result["latest_event"]["ended_at"], "resolved");
        assert_eq!(result["latest_event"]["affected_services"], json!(["AUR"]));
        assert_eq!(result["latest_down"], Value::Null);
    }
}

/// 【插件测试】【空监控】无事件和无监控时返回明确的未知状态及 null。
#[tokio::test]
async fn arch_status_preserves_nulls_when_no_monitor_exists() {
    let host = Arc::new(FixtureHost::new(&[(200, "{}"), (200, r#"{"data":[]}"#)]));
    let plugin = runtime("archlinux", host.clone());
    let result = call_json(&plugin, "aur_check_status", json!({})).await;
    assert_eq!(
        result,
        json!({
            "success":true,"current_state":"unknown","is_degraded":false,"degraded_reason":null,
            "latest_down":null,"latest_event":null,
            "monitor":{"name":"AUR","status_class":"","monitor_id":null},
            "source":"https://status.archlinux.org",
        })
    );
    assert_eq!(host.requests.lock().unwrap().len(), 2);
}

/// 【插件测试】【降级判定】无关事件不影响 AUR，告警状态与故障状态分别映射。
#[tokio::test]
async fn arch_status_distinguishes_monitor_warning_from_unrelated_incidents() {
    let events = r#"{"results":[{"title":"Mirror issue","content":"Affected services:\n- mirror.archlinux.org"}]}"#;
    for (status, state, degraded) in [
        ("warning", "unknown", true),
        ("DEGRADED", "unknown", true),
        ("error", "down", false),
        ("down", "down", false),
        ("success", "up", false),
    ] {
        let monitors = json!({"data":[{"name":"Fallback","statusClass":status}]}).to_string();
        let host = Arc::new(FixtureHost::new(&[(200, events), (200, &monitors)]));
        let plugin = runtime("archlinux", host);
        let result = call_json(&plugin, "aur_check_status", json!({})).await;
        assert_eq!(result["current_state"], state);
        assert_eq!(result["is_degraded"], degraded);
        assert_eq!(result["monitor"]["name"], "Fallback");
        if degraded {
            assert_eq!(
                result["degraded_reason"],
                "AUR monitor status is not fully healthy"
            );
        }
    }
}
