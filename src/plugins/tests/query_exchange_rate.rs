use super::query_support::{runtime, QueryHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【汇率插件测试】【有效数字】接口数值经 JSON 与 Lua 后仍使用原版 f64 十进制展示。
#[tokio::test]
async fn exchange_rates_preserve_float_formatting_and_chinese_aliases() {
    for value in [
        0.0,
        -0.0,
        1.0,
        7.123456789012345,
        1e-20,
        1e20,
        f64::MIN_POSITIVE,
    ] {
        let response = json!({"rates":{"CNY":value}}).to_string();
        let host = Arc::new(QueryHost::new(&[Ok((200, &response))]));
        let plugin = runtime("exchange-rate", json!({}), host.clone());
        let output = plugin
            .call_tool(
                "get_exchange_rate",
                json!({"base":"　美金 ","target":"人民币"}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(output, format!("USD 到 CNY 的汇率是: {value}"));
        assert_eq!(
            host.requests.lock().unwrap()[0].url,
            "https://open.er-api.com/v6/latest/USD"
        );
    }
}

/// 【汇率插件测试】【收费优先】成功的收费查询不访问免费接口，密钥作为单个路径参数编码。
#[tokio::test]
async fn configured_exchange_rates_short_circuit_the_free_provider() {
    let host = Arc::new(QueryHost::new(&[Ok((
        200,
        r#"{"result":"success","conversion_rates":{"JPY":149.25}}"#,
    ))]));
    let plugin = runtime(
        "exchange-rate",
        json!({"api_key":" key/with?characters ","free_fallback_enabled":false}),
        host.clone(),
    );
    let output = plugin
        .call_tool(
            "get_exchange_rate",
            json!({"base":"usd","target":"日元"}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(output, "USD 到 JPY 的汇率是: 149.25");
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url,
        "https://v6.exchangerate-api.com/v6/key%2Fwith%3Fcharacters/latest/USD"
    );
    assert!(requests[0].body.is_none());
}

/// 【汇率插件测试】【业务回退】有效 JSON 缺少成功汇率时才访问免费接口，错误类型不能冒充汇率。
#[tokio::test]
async fn exchange_rate_business_failures_use_the_enabled_fallback() {
    for first in [
        Value::Null,
        json!(false),
        json!({"result":"error"}),
        json!({"result":"success","conversion_rates":{}}),
        json!({"result":"success","conversion_rates":{"EUR":"0.9"}}),
        json!({"result":"success","conversion_rates":{"EUR":false}}),
        json!({"result":"success","conversion_rates":{"EUR":null}}),
    ] {
        let first = first.to_string();
        let host = Arc::new(QueryHost::new(&[
            Ok((200, &first)),
            Ok((200, r#"{"rates":{"EUR":0.9}}"#)),
        ]));
        let plugin = runtime(
            "exchange-rate",
            json!({"api_key":"fixture-key"}),
            host.clone(),
        );
        let output = plugin
            .call_tool(
                "get_exchange_rate",
                json!({"base":"美元","target":"欧元"}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(output, "USD 到 EUR 的汇率是: 0.9");
        let requests = host.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].url, "https://open.er-api.com/v6/latest/USD");
    }
}

/// 【汇率插件测试】【请求失败】HTTP、传输和 JSON 解析错误沿用直接失败语义，不隐式回退或泄漏密钥。
#[tokio::test]
async fn exchange_rate_transport_status_and_json_errors_do_not_fall_back() {
    for response in [
        Ok((401, "fixture-private-key")),
        Ok((200, "not JSON")),
        Err("connection fixture failed"),
    ] {
        let host = Arc::new(QueryHost::new(&[
            response,
            Ok((200, r#"{"rates":{"EUR":0.9}}"#)),
        ]));
        let plugin = runtime(
            "exchange-rate",
            json!({"api_key":"fixture-private-key"}),
            host.clone(),
        );
        let error = plugin
            .call_tool(
                "get_exchange_rate",
                json!({"base":"USD","target":"EUR"}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(!format!("{error:#}").contains("fixture-private-key"));
        assert_eq!(host.requests.lock().unwrap().len(), 1);
    }
}

/// 【汇率插件测试】【显式关闭】false 必须关闭免费回退，缺少密钥时不能访问任何来源。
#[tokio::test]
async fn disabled_exchange_fallback_and_empty_currencies_are_respected() {
    let host = Arc::new(QueryHost::new(&[]));
    let plugin = runtime(
        "exchange-rate",
        json!({"free_fallback_enabled":false}),
        host.clone(),
    );
    let error = plugin
        .call_tool(
            "get_exchange_rate",
            json!({"base":"USD","target":"EUR"}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("free fallback is disabled"));
    for arguments in [
        json!({"base":"　","target":"EUR"}),
        json!({"base":"USD","target":" "}),
    ] {
        let error = plugin
            .call_tool("get_exchange_rate", arguments, InvocationContext::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("base and target are required"));
    }
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【汇率插件测试】【目标缺失】免费接口缺少数值时保留原错误，不把字符串或布尔值转成数值。
#[tokio::test]
async fn exchange_rate_missing_targets_fail_explicitly() {
    for value in [
        json!(false),
        Value::Null,
        json!({}),
        json!({"rates":{"JPY":"2"}}),
        json!({"rates":{"JPY":false}}),
    ] {
        let response = value.to_string();
        let plugin = runtime(
            "exchange-rate",
            json!({}),
            Arc::new(QueryHost::new(&[Ok((200, &response))])),
        );
        let error = plugin
            .call_tool(
                "get_exchange_rate",
                json!({"base":"USD","target":"JPY"}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("target currency not found: JPY"));
    }
}
