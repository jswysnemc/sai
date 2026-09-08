use super::support::{call_json, runtime, FixtureHost};
use sai_plugin_runtime::InvocationContext;
use serde_json::{json, Value};
use std::sync::Arc;

/// 【插件测试】【AUR 搜索】兼容字段、null 和时间格式，并编码非 ASCII 查询。
#[tokio::test]
async fn aur_search_preserves_fields_and_encodes_queries() {
    let data = json!({"results":[{
        "Name":"输入法", "PackageBase":"input-base", "Version":"1.2", "Description":null,
        "NumVotes":12, "Popularity":0.5, "Maintainer":null, "OutOfDate":0,
        "LastModified":1735689600, "URL":"https://example.test"
    }]})
    .to_string();
    let host = Arc::new(FixtureHost::new(&[(200, &data)]));
    let plugin = runtime("archlinux", host.clone());
    let result = call_json(
        &plugin,
        "aur_search_packages",
        json!({"query":"　输入 +  ", "search_by":"name-desc"}),
    )
    .await;
    assert_eq!(
        result,
        json!({
            "success":true, "query":"输入 +", "results":[{
                "name":"输入法", "package_base":"input-base", "version":"1.2", "description":null,
                "votes":12, "popularity":0.5, "maintainer":null,
                "out_of_date":true, "out_of_date_at":0, "last_modified":1735689600,
                "last_modified_iso":"2025-01-01T00:00:00+00:00", "upstream_url":"https://example.test",
                "aur_url":"https://aur.archlinux.org/packages/输入法",
            }]
        })
    );
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].url,
        "https://aur.archlinux.org/rpc/?v=5&type=search&by=name-desc&arg=%E8%BE%93%E5%85%A5%20%2B"
    );
}

/// 【插件测试】【AUR 数量】默认十项、最多五十项，零和空结果仍使用 JSON 数组。
#[tokio::test]
async fn aur_search_obeys_limits_and_preserves_empty_arrays() {
    let data = json!({"results": vec![json!({}); 55]}).to_string();
    let host = Arc::new(FixtureHost::new(&[
        (200, &data),
        (200, &data),
        (200, &data),
        (200, &data),
        (200, "{}"),
    ]));
    let plugin = runtime("archlinux", host);
    for (args, count) in [
        (json!({"query":"test"}), 10),
        (json!({"query":"test","limit":100}), 50),
        (json!({"query":"test","limit":-1}), 10),
        (json!({"query":"test","limit":0}), 0),
        (json!({"query":"test"}), 0),
    ] {
        let result = call_json(&plugin, "aur_search_packages", args).await;
        assert_eq!(result["results"].as_array().unwrap().len(), count);
        if count > 0 {
            assert_eq!(result["results"][0]["name"], "");
            assert_eq!(result["results"][0]["out_of_date"], false);
            assert_eq!(result["results"][0]["aur_url"], Value::Null);
            assert_eq!(result["results"][0]["last_modified_iso"], Value::Null);
        }
    }
}

/// 【插件测试】【AUR 详情】保留五包上限、大小写匹配、单值依赖和空列表。
#[tokio::test]
async fn aur_info_preserves_requested_found_missing_and_dependencies() {
    let data = json!({"results":[{
        "Name":"Yay", "License":"GPL", "Keywords":[], "Depends":["pacman"],
        "MakeDepends":null, "CheckDepends":false, "OptDepends":{"sample":"value"},
        "FirstSubmitted":0, "LastModified":"bad timestamp", "URLPath":"/snapshot/yay.tar.gz"
    }]})
    .to_string();
    let host = Arc::new(FixtureHost::new(&[(200, &data), (200, "{}")]));
    let plugin = runtime("archlinux", host.clone());
    let result = call_json(
        &plugin,
        "aur_get_package_info",
        json!({
            "package_name":"yay, missing 包名 third fourth ignored"
        }),
    )
    .await;
    assert_eq!(
        result["requested"],
        json!(["yay", "missing", "包名", "third", "fourth"])
    );
    assert_eq!(result["found"], json!(["Yay"]));
    assert_eq!(
        result["missing"],
        json!(["missing", "包名", "third", "fourth"])
    );
    let item = &result["results"][0];
    assert_eq!(item["license"], json!(["GPL"]));
    assert_eq!(item["keywords"], json!([]));
    assert_eq!(item["depends"], json!(["pacman"]));
    assert_eq!(item["make_depends"], json!([]));
    assert_eq!(item["check_depends"], json!([false]));
    assert_eq!(item["opt_depends"], json!([{"sample":"value"}]));
    assert_eq!(item["provides"], json!([]));
    assert_eq!(item["conflicts"], json!([]));
    assert_eq!(item["first_submitted_iso"], "1970-01-01T00:00:00+00:00");
    assert_eq!(item["last_modified_iso"], Value::Null);
    assert_eq!(item["url_path"], "/snapshot/yay.tar.gz");
    assert_eq!(host.requests.lock().unwrap()[0].url,
        "https://aur.archlinux.org/rpc/?v=5&type=info&arg[]=yay&arg[]=missing&arg[]=%E5%8C%85%E5%90%8D&arg[]=third&arg[]=fourth");
    let empty = call_json(
        &plugin,
        "aur_get_package_info",
        json!({"package_name":"missing"}),
    )
    .await;
    assert_eq!(empty["found"], json!([]));
    assert_eq!(empty["results"], json!([]));
    assert_eq!(empty["missing"], json!(["missing"]));
}

/// 【插件测试】【官方包】自动模式依据仓库选择接口，保留源数据与编码。
#[tokio::test]
async fn official_packages_choose_search_or_details_from_arguments() {
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"{"results":[{"pkgname":"linux"}]}"#),
        (200, r#"{"pkgname":"包 名"}"#),
        (200, "{}"),
    ]));
    let plugin = runtime("archlinux", host.clone());
    let search = call_json(
        &plugin,
        "archlinux_official_package_query",
        json!({"package_name":" linux "}),
    )
    .await;
    assert_eq!(search["mode"], "search");
    assert_eq!(search["repo"], Value::Null);
    assert_eq!(search["arch"], "x86_64");
    assert_eq!(search["data"]["results"][0]["pkgname"], "linux");
    let detail = call_json(
        &plugin,
        "archlinux_official_package_query",
        json!({"package_name":"包 名", "repo":" extra ", "arch":" any "}),
    )
    .await;
    assert_eq!(detail["mode"], "detail");
    assert_eq!(detail["repo"], "extra");
    assert_eq!(detail["arch"], "any");
    assert_eq!(detail["data"], json!({"pkgname":"包 名"}));
    call_json(
        &plugin,
        "archlinux_official_package_query",
        json!({"package_name":"linux", "repo":"extra", "mode":"search"}),
    )
    .await;
    let requests = host.requests.lock().unwrap();
    assert_eq!(
        requests[0].url,
        "https://archlinux.org/packages/search/json/?name=linux"
    );
    assert_eq!(
        requests[1].url,
        "https://archlinux.org/packages/extra/any/%E5%8C%85%20%E5%90%8D/json/"
    );
    assert_eq!(requests[2].url, requests[0].url);
}

/// 【插件测试】【无效请求】参数错误在网络访问前失败，远端错误不伪装为成功。
#[tokio::test]
async fn arch_queries_reject_invalid_arguments_and_http_failures() {
    let host = Arc::new(FixtureHost::new(&[(503, "unavailable")]));
    let plugin = runtime("archlinux", host.clone());
    for (name, args) in [
        ("aur_search_packages", json!({"query":"  "})),
        ("aur_get_package_info", json!({"package_name":", ,"})),
        (
            "archlinux_official_package_query",
            json!({"package_name":"linux", "mode":"detail"}),
        ),
        ("archwiki_query", json!({"title":"title", "mode":"invalid"})),
    ] {
        assert!(plugin
            .call_tool(name, args, InvocationContext::default())
            .await
            .is_err());
    }
    assert!(host.requests.lock().unwrap().is_empty());
    let error = plugin
        .call_tool(
            "aur_search_packages",
            json!({"query":"linux"}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("HTTP 503"));
}

/// 【插件测试】【Wiki 搜索】保留原始 opensearch 数组与嵌套空数组。
#[tokio::test]
async fn archwiki_search_returns_the_original_search_document() {
    let data = json!([
        "输入法",
        ["Fcitx5"],
        [],
        ["https://wiki.archlinux.org/title/Fcitx5"]
    ])
    .to_string();
    let host = Arc::new(FixtureHost::new(&[(200, &data)]));
    let plugin = runtime("archlinux", host.clone());
    let result = call_json(
        &plugin,
        "archwiki_query",
        json!({"query":"输入法", "mode":"search"}),
    )
    .await;
    assert_eq!(result, serde_json::from_str::<Value>(&data).unwrap());
    assert_eq!(host.requests.lock().unwrap()[0].url,
        "https://wiki.archlinux.org/api.php?action=opensearch&search=%E8%BE%93%E5%85%A5%E6%B3%95&limit=8&namespace=0&format=json");
}

/// 【插件测试】【Wiki 页面】自动读取首项并保留链接，指定标题时跳过搜索。
#[tokio::test]
async fn archwiki_auto_and_page_modes_preserve_markdown_links() {
    let html = json!({"parse":{"text":{"*":"<h2>输入法</h2><p>使用 <a href=\"/title/Fcitx5\">Fcitx5</a></p>"}}}).to_string();
    let host = Arc::new(FixtureHost::new(&[
        (200, r#"["input",["Fcitx5 (简体中文)"],[],[]]"#),
        (200, &html),
        (200, &html),
        (200, r#"["input",[],[],[]]"#),
        (200, &html),
    ]));
    let plugin = runtime("archlinux", host.clone());
    for args in [
        json!({"query":"input"}),
        json!({"title":"Explicit title", "query":"ignored"}),
        json!({"query":"Fallback title"}),
    ] {
        let output = plugin
            .call_tool("archwiki_query", args, InvocationContext::default())
            .await
            .unwrap();
        assert!(output.contains("输入法"));
        assert!(output.contains("[Fcitx5](/title/Fcitx5)"));
    }
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(requests[1]
        .url
        .contains("page=Fcitx5%20%28%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87%29"));
    assert!(requests[2].url.contains("page=Explicit%20title"));
    assert!(requests[4].url.contains("page=Fallback%20title"));
}
