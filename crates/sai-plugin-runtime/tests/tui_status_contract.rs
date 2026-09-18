mod common;

use sai_plugin_runtime::{Capabilities, TuiStatusContext, TuiStatusRuntime};
use serde_json::json;

/// 【底栏契约测试】【状态样本】创建不含会话正文的显示数据。
/// @returns 固定终端状态
fn context() -> TuiStatusContext {
    TuiStatusContext {
        columns: 80,
        locale: "zh-CN".into(),
        mode: "audit".into(),
        model: "test-model".into(),
        thinking: "high".into(),
        directory: "~/project".into(),
        context_ratio: 0.25,
        context_window_tokens: 128000,
        cache_hit_ratio: Some(0.9),
    }
}

/// 【底栏契约测试】【纯实例】从测试源码创建仅授予底栏能力的运行时。
/// @param source Lua 源码；返回真实运行实例
fn runtime(source: &str) -> TuiStatusRuntime {
    let mut package = common::package(source);
    package.manifest.capabilities.tui_status = true;
    TuiStatusRuntime::load(
        package,
        json!({}),
        Capabilities {
            tui_status: true,
            ..Default::default()
        },
    )
    .unwrap()
}

/// 【底栏契约测试】【独立授权】通知授权不能替代底栏授权，缺少任一侧均不得加载。
#[test]
fn tui_status_requires_its_own_declaration_and_grant() {
    for (declared, granted) in [(true, false), (false, true), (false, false)] {
        let mut package = common::package("error('must not initialize')");
        package.manifest.capabilities.tui_status = declared;
        let grants = Capabilities {
            tui_status: granted,
            notifications: true,
            ..Default::default()
        };
        assert_eq!(
            TuiStatusRuntime::load(package, json!({}), grants)
                .err()
                .unwrap()
                .to_string(),
            "plugin TUI status capability is not allowed"
        );
    }
    let declared = Capabilities {
        tui_status: true,
        ..Default::default()
    };
    assert!(!declared.intersection(&Capabilities::default()).tui_status);
    assert!(!declared.is_subset(&Capabilities::default()));
    assert!(declared.is_subset(&declared));
}

/// 【底栏契约测试】【宿主隔离】其他授权不进入展示实例，回调只能返回纯文本。
#[tokio::test]
async fn tui_status_callbacks_have_no_io_or_private_context() {
    let mut package = common::package(
        r#"
        sai.on('tui_status', function(event, ctx)
            assert(ctx.session_id == '' and ctx.workdir == '' and not ctx.allow_writes)
            assert(io == nil and os == nil and print == nil)
            for _, request in ipairs({
                function() return sai.http.request({url='https://example.test'}) end,
                function() return sai.fs.read_text('secret') end,
                function() return sai.storage.get('secret') end,
                function() return sai.model.complete({messages={{role='user',content='secret'}}}) end,
                function() return sai.terminal.size() end,
            }) do assert(not pcall(request)) end
            return {left=event.mode .. ':' .. event.model, right=event.directory}
        end)
    "#,
    );
    package.manifest.capabilities = serde_json::from_value(json!({
        "tui_status":true, "model":true, "http":["https://example.test"],
        "system":{"read_paths":["."], "session_storage":true}
    }))
    .unwrap();
    let grants = package.manifest.capabilities.clone();
    let runtime = TuiStatusRuntime::load(package, json!({}), grants).unwrap();
    let line = runtime.render(&context()).await.unwrap().unwrap();
    assert_eq!(line.left, "audit:test-model");
    assert_eq!(line.right, "~/project");
}

/// 【底栏契约测试】【非法结果】终端控制码、换行、超长结果及多布局均不能进入绘制。
#[tokio::test]
async fn tui_status_rejects_unsafe_or_ambiguous_output() {
    for value in [
        json!(false),
        json!({"left":"text"}),
        json!({"left":"\u{001b}[2J", "right":""}),
        json!({"left":"a\nb", "right":""}),
        json!({"left":"a", "right":"\u{0085}"}),
        json!({"left":"a", "right":"\u{2028}"}),
        json!({"left":"a".repeat(2049), "right":""}),
        json!({"left":"a", "right":"", "command":"anything"}),
    ] {
        let source =
            format!("sai.on('tui_status', function() return sai.json.decode([==[{value}]==]) end)");
        assert!(runtime(&source).render(&context()).await.is_err());
    }
    assert!(runtime(
        "for i=1,2 do sai.on('tui_status', function() return {left='x',right=''} end) end"
    )
    .render(&context())
    .await
    .is_err());
    assert!(runtime("sai.on('tui_status', function() return nil end)")
        .render(&context())
        .await
        .unwrap()
        .is_none());
}

/// 【底栏契约测试】【超时终止】无限循环受预算终止，后续调用仍可完成。
#[tokio::test]
async fn tui_status_runaway_callback_is_bounded_and_recoverable() {
    let runtime = runtime("local first=true; sai.on('tui_status', function() if first then first=false; while true do end end return {left='recovered',right=''} end)");
    assert!(tokio::time::timeout(
        std::time::Duration::from_secs(2),
        runtime.render(&context())
    )
    .await
    .unwrap()
    .is_err());
    assert_eq!(
        runtime.render(&context()).await.unwrap().unwrap().left,
        "recovered"
    );
}
