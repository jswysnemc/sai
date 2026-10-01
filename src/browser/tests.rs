use super::keys::{editing_commands, parse_chord};
use super::launcher::{launch_args, parse_active_port};
use super::snapshot::{element_expression, snapshot_script};
use super::url_policy::normalize_url;

/// 【内置浏览器测试】【地址补全】无协议主机按本机或公网补全协议，搜索词只在允许时转换。
#[test]
fn normalize_url_completes_scheme_and_search() {
    assert_eq!(
        normalize_url("example.com", false).unwrap(),
        "https://example.com"
    );
    assert_eq!(
        normalize_url("localhost:5173/app", false).unwrap(),
        "http://localhost:5173/app"
    );
    assert_eq!(
        normalize_url("192.168.1.8:8080", false).unwrap(),
        "http://192.168.1.8:8080"
    );
    assert_eq!(
        normalize_url(" https://a.b/c?d=1 ", false).unwrap(),
        "https://a.b/c?d=1"
    );
    assert_eq!(normalize_url("about:blank", false).unwrap(), "about:blank");
    assert!(normalize_url("rust async", false).is_err());
    assert_eq!(
        normalize_url("rust async", true).unwrap(),
        "https://duckduckgo.com/?q=rust%20async"
    );
}

/// 【内置浏览器测试】【协议拦截】本地文件、脚本与浏览器内部页面一律拒绝。
#[test]
fn normalize_url_blocks_local_and_internal_schemes() {
    for input in [
        "file:///etc/passwd",
        "chrome://settings",
        "javascript:alert(1)",
        "devtools://devtools/bundled/inspector.html",
        "view-source:https://example.com",
        "data:text/html,<b>x</b>",
        "ftp://example.com",
    ] {
        assert!(
            normalize_url(input, true).is_err(),
            "{input} must be blocked"
        );
    }
}

/// 【内置浏览器测试】【端口文件】只接受完整的端口与浏览器路径。
#[test]
fn active_port_file_parses_into_websocket_url() {
    assert_eq!(
        parse_active_port("41235\n/devtools/browser/abc-123\n").as_deref(),
        Some("ws://127.0.0.1:41235/devtools/browser/abc-123")
    );
    assert_eq!(parse_active_port("41235\n"), None);
    assert_eq!(parse_active_port("0\n/devtools/browser/x"), None);
    assert_eq!(parse_active_port(""), None);
}

/// 【内置浏览器测试】【启动参数】调试端口只监听本机，无头模式可切换，用户目录隔离。
#[test]
fn launch_args_bind_loopback_and_isolate_profile() {
    let profile = std::path::Path::new("/tmp/sai-browser-test");
    let args = launch_args(profile, false, None);
    assert!(args.contains(&"--remote-debugging-address=127.0.0.1".to_string()));
    assert!(args.contains(&"--remote-debugging-port=0".to_string()));
    assert!(args.contains(&"--headless=new".to_string()));
    assert!(!args.contains(&"--hide-scrollbars".to_string()));
    assert!(args
        .iter()
        .any(|arg| arg == "--user-data-dir=/tmp/sai-browser-test"));
    assert!(!launch_args(profile, true, None).contains(&"--headless=new".to_string()));
    assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_string()));
    let spoofed = launch_args(profile, false, Some("Mozilla/5.0 Chrome/150"));
    assert!(spoofed.contains(&"--user-agent=Mozilla/5.0 Chrome/150".to_string()));
}

/// 【内置浏览器测试】【组合键】修饰键累加、单字符与功能键都能解析。
#[test]
fn chords_parse_modifiers_and_keys() {
    let chord = parse_chord("Control+Shift+K").unwrap();
    assert_eq!(chord.modifiers, 2 | 8);
    assert_eq!(chord.key.key, "K");
    assert_eq!(chord.key.code, "KeyK");
    let enter = parse_chord("enter").unwrap();
    assert_eq!(enter.key.key, "Enter");
    assert_eq!(enter.key.text.as_deref(), Some("\r"));
    assert_eq!(parse_chord("F5").unwrap().key.key_code, 116);
    assert_eq!(parse_chord("+").unwrap().key.key, "+");
    assert!(parse_chord("Hyper+A").is_err());
    assert!(parse_chord("NotAKey").is_err());
    assert_eq!(editing_commands(2, "a"), vec!["selectAll"]);
    assert_eq!(editing_commands(2 | 8, "z"), vec!["redo"]);
    assert!(editing_commands(0, "a").is_empty());
}

/// 【内置浏览器测试】【ref 校验】只接受快照生成的 eN 形式，防止任意脚本拼接。
#[test]
fn element_expression_rejects_malformed_refs() {
    assert!(element_expression("e12").unwrap().contains("\"e12\""));
    assert!(element_expression("ref=e3").unwrap().contains("\"e3\""));
    for bad in ["", "12", "e", "e1'); alert(1); ('", "x9"] {
        assert!(element_expression(bad).is_err(), "{bad:?} must be rejected");
    }
}

/// 【内置浏览器测试】【快照脚本】占位符全部替换，脚本是一个可求值表达式。
#[test]
fn snapshot_script_replaces_placeholders() {
    let script = snapshot_script(5000, true);
    assert!(!script.contains("__MAX_CHARS__"));
    assert!(!script.contains("__INTERACTIVE_ONLY__"));
    assert!(script.contains("const MAX_CHARS = 5000;"));
    assert!(script.contains("const INTERACTIVE_ONLY = true;"));
    assert!(script.trim_end().ends_with("})()"));
}

/// 【内置浏览器测试】【真实浏览器】设置 SAI_BROWSER_E2E=1 时启动本机浏览器完成一轮完整操作。
#[tokio::test]
async fn real_browser_round_trip_when_enabled() {
    if std::env::var("SAI_BROWSER_E2E").ok().as_deref() != Some("1") {
        return;
    }
    let page = "data:text/html,<title>Sai</title><h1>Hello</h1>\
        <input id=q placeholder=Query><button onclick=\"document.title=document.getElementById('q').value\">Go</button>\
        <a href='/next'>Next</a>";
    let session = super::BrowserSession::launch_with(super::ProfileMode::Temporary)
        .await
        .unwrap();
    // data: 地址只在测试中直接导航，工具入口的地址策略会拦截
    session.navigate(page).await.unwrap();
    let snapshot = session.snapshot(20_000, false).await.unwrap();
    let text = snapshot.render();
    assert!(text.contains("heading \"Hello\" [level=1]"), "{text}");
    let textbox = find_ref(&snapshot.text, "textbox \"Query\"");
    let button = find_ref(&snapshot.text, "button \"Go\"");
    session
        .type_text(&textbox, "你好 sai", true, false)
        .await
        .unwrap();
    session
        .click(&button, super::actions::ClickKind::Single)
        .await
        .unwrap();
    let title = session.evaluate("document.title").await.unwrap();
    assert_eq!(title, "你好 sai");
    // ref 表在隔离世界里：页面主世界既看不到，也无法用伪造的表劫持点击
    let leaked = session.evaluate("typeof window.__saiRefs").await.unwrap();
    assert_eq!(leaked, "undefined");
    session
        .evaluate(
            "window.__saiRefs = new Map([[\"e1\", document.querySelector('a')]]); \
             document.querySelector('a').onclick = () => { document.title = 'hijacked'; }; true",
        )
        .await
        .unwrap();
    session
        .click(&button, super::actions::ClickKind::Single)
        .await
        .unwrap();
    assert_eq!(
        session.evaluate("document.title").await.unwrap(),
        "你好 sai"
    );
    // 换文档后旧 ref 失效，错误提示重新快照
    session
        .navigate("data:text/html,<button>Other</button>")
        .await
        .unwrap();
    let stale = session
        .click(&button, super::actions::ClickKind::Single)
        .await
        .unwrap_err();
    assert!(
        format!("{stale:#}").contains("browser with action=snapshot"),
        "{stale:#}"
    );
    let (jpeg, _) = session.screenshot(false, None).await.unwrap();
    assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    let tab = session.new_tab("about:blank").await.unwrap();
    assert_eq!(session.list_tabs().await.unwrap().len(), 2);
    session.close_tab(Some(&tab)).await.unwrap();
    assert_eq!(session.list_tabs().await.unwrap().len(), 1);
    session.close().await;
}

/// 【内置浏览器测试】【ref 提取】从快照文本中找出指定条目的 ref。
/// @param text 为快照文本；prefix 为条目开头
/// @returns ref 字符串
fn find_ref(text: &str, prefix: &str) -> String {
    let line = text
        .lines()
        .find(|line| {
            line.trim_start()
                .trim_start_matches("- ")
                .starts_with(prefix)
        })
        .unwrap_or_else(|| panic!("{prefix} not in snapshot:\n{text}"));
    let start = line.find("[ref=").unwrap() + 5;
    let end = line[start..].find(']').unwrap() + start;
    line[start..end].to_string()
}

/// 【内置浏览器测试】【无头标识】只改写无头标识，普通 User-Agent 保持原样。
#[test]
fn headless_user_agent_is_rewritten_to_chrome() {
    use super::profile::normal_user_agent;
    assert_eq!(
        normal_user_agent("Mozilla/5.0 (X11) HeadlessChrome/154.0.0.0 Safari/537.36").as_deref(),
        Some("Mozilla/5.0 (X11) Chrome/154.0.0.0 Safari/537.36")
    );
    assert_eq!(normal_user_agent("Mozilla/5.0 Chrome/154.0.0.0"), None);
}

/// 【内置浏览器测试】【下载文件名】去掉路径与引号，防止写出目录外或注入响应头。
#[test]
fn download_file_names_are_sanitized() {
    use super::downloads::sanitize_file_name;
    assert_eq!(sanitize_file_name("../../etc/passwd"), "etcpasswd");
    assert_eq!(sanitize_file_name("a\"b\r\n.zip"), "ab.zip");
    assert_eq!(sanitize_file_name("  ..  "), "download");
    assert_eq!(sanitize_file_name("报告 2026.pdf"), "报告 2026.pdf");
}

/// 【内置浏览器测试】【上传 ID】只接受本进程生成的 ID，拒绝路径穿越。
#[tokio::test]
async fn upload_ids_cannot_escape_the_upload_directory() {
    let id = super::store_upload("a.txt", b"x").unwrap();
    assert_eq!(id.len(), 32);
    let session_less = super::uploads::resolve_upload_for_test(&id).unwrap();
    assert!(session_less.ends_with("a.txt"));
    for bad in ["../etc", "", "zzzz", &"a".repeat(31)] {
        assert!(
            super::uploads::resolve_upload_for_test(bad).is_err(),
            "{bad}"
        );
    }
    assert!(super::store_upload("big", &vec![0; super::MAX_UPLOAD_BYTES + 1]).is_err());
    super::uploads::remove_uploads();
}

/// 【内置浏览器测试】【调试端口】从浏览器调试地址取出端口。
#[test]
fn debug_port_is_parsed_from_the_websocket_url() {
    use super::session_lifecycle::debug_port;
    assert_eq!(debug_port("ws://127.0.0.1:41235/devtools/browser/x"), 41235);
    assert_eq!(debug_port("ws://example.com/x"), 0);
}

/// 【内置浏览器测试】【下载记录】残留的浏览器下载记录在启动前被清空，其余历史保留。
#[test]
fn stale_download_history_is_cleared_before_launch() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("Default");
    std::fs::create_dir_all(&history).unwrap();
    let connection = rusqlite::Connection::open(history.join("History")).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE downloads (id INTEGER PRIMARY KEY, state INTEGER);
             CREATE TABLE downloads_url_chains (id INTEGER, url TEXT);
             CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT);
             INSERT INTO downloads VALUES (1, 0);
             INSERT INTO downloads_url_chains VALUES (1, 'http://x');
             INSERT INTO urls VALUES (1, 'http://kept');",
        )
        .unwrap();
    drop(connection);
    super::profile::clear_download_history(root.path());
    let connection = rusqlite::Connection::open(history.join("History")).unwrap();
    let count = |table: &str| -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!(count("downloads"), 0);
    assert_eq!(count("downloads_url_chains"), 0);
    assert_eq!(count("urls"), 1, "普通浏览历史保留");
}
