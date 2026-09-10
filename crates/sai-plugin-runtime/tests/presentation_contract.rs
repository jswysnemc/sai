mod common;

use sai_plugin_runtime::{
    Capabilities, Notification, PresentationRuntime, PresentationSurface, ReplyPresentation,
    ReplyStatus,
};
use serde_json::json;

/// 【展示契约测试】【事件样本】提供不包含会话数据的有限事件。
/// @returns Web 中文完成事件
fn event() -> ReplyPresentation {
    ReplyPresentation {
        surface: PresentationSurface::Web,
        status: ReplyStatus::Completed,
        locale: "zh-CN".into(),
    }
}

/// 【展示契约测试】【实例】使用真实纯运行时加载通知脚本。
/// @param source Lua 源码
/// @returns 已声明并授权通知的运行时
fn runtime(source: &str) -> PresentationRuntime {
    let mut package = common::package(source);
    package.manifest.capabilities.notifications = true;
    let granted = package.manifest.capabilities.clone();
    PresentationRuntime::load(package, json!({}), granted).unwrap()
}

/// 【展示契约测试】【授权交集】通知能力不能通过网络或模型授权间接取得。
#[test]
fn notification_grants_are_explicit_and_independent() {
    let declared: Capabilities =
        serde_json::from_value(json!({"notifications":true,"model":true})).unwrap();
    let mut granted: Capabilities = serde_json::from_value(json!({"model":true})).unwrap();
    assert!(!declared.intersection(&granted).notifications);
    granted.notifications = true;
    assert!(declared.intersection(&granted).notifications);
    assert!(granted.is_subset(&declared));
    assert!(!granted.is_subset(&Capabilities::default()));
    assert!(serde_json::from_value::<Capabilities>(json!({"notifications":"true"})).is_err());
}

/// 【展示契约测试】【加载拒绝】缺少声明或授权时不执行任何初始化代码。
#[test]
fn presentation_loading_requires_both_declaration_and_grant() {
    for (declared, granted) in [(false, false), (false, true), (true, false)] {
        let mut package = common::package("error('initialization must not run')");
        package.manifest.capabilities.notifications = declared;
        let result = PresentationRuntime::load(
            package,
            json!({}),
            Capabilities {
                notifications: granted,
                ..Default::default()
            },
        );
        let error = result.err().expect("missing authorization");
        assert_eq!(
            error.to_string(),
            "plugin notification capability is not allowed"
        );
    }
}

/// 【展示契约测试】【纯回调】已有其他能力授权也不能在通知计算中执行宿主 I/O。
#[tokio::test]
async fn presentation_callbacks_cannot_use_other_host_capabilities_or_session_data() {
    let mut package = common::package(
        r#"
        sai.on('reply_end', function(event, ctx)
            assert(ctx.session_id == '' and ctx.workdir == '' and ctx.allow_writes == false)
            ctx.allow_writes = true
            ctx.workdir = '/'
            assert(type(sai.process.output) == 'function' and type(sai.workspace.open) == 'function')
            local checks = {
                function() return sai.http.request({url='https://example.test'}) end,
                function() return sai.env.get('LANG') end,
                function() return sai.fs.read_text('secret') end,
                function() return sai.process.output('read', {}) end,
                function() return sai.storage.get('secret') end,
                function() return sai.workspace.open('directory') end,
                function() return sai.model.complete({messages={{role='user',content='secret'}}}) end,
                function() return sai.tools.call('read_file', {path='secret'}) end,
            }
            for _, check in ipairs(checks) do
                local ok, reason = pcall(check)
                assert(not ok)
                reason = tostring(reason)
                assert(reason:find('not allowed', 1, true) or reason:find('unavailable', 1, true), reason)
            end
            assert(io == nil and os == nil and coroutine == nil)
            return {title='Pure',body=event.status..':'..event.surface..':'..event.locale,desktop=true,sound=false}
        end)
        "#,
    );
    package.manifest.capabilities = serde_json::from_value(json!({
        "notifications":true,"model":true,"tools":["read_file"],
        "http":["https://example.test"],
        "system": {
            "read_paths":["."],"environment":["LANG"],"session_storage":true,"workspace":true,
            "processes":{"read":{"program":"read-fixture","read_only":true}}
        }
    }))
    .unwrap();
    let granted = package.manifest.capabilities.clone();
    let runtime = PresentationRuntime::load(package, json!({}), granted).unwrap();
    assert_eq!(
        runtime.reply_end(&event()).await.unwrap(),
        [Notification {
            title: "Pure".into(),
            body: "completed:web:zh-CN".into(),
            desktop: true,
            sound: false,
        }]
    );
}

/// 【展示契约测试】【原子结果】一个监听器返回非法数据时不交付同包的部分结果。
#[tokio::test]
async fn invalid_notification_results_reject_the_entire_plugin_batch() {
    for invalid in [
        json!(false),
        json!(42),
        json!({"title":"valid","body":"text"}),
        json!({"title":"valid","body":"text","desktop":"false","sound":false}),
        json!({"title":" ","body":"text","desktop":true,"sound":false}),
        json!({"title":"x".repeat(257),"body":"text","desktop":true,"sound":false}),
        json!({"title":"valid","body":"界".repeat(1366),"desktop":true,"sound":false}),
        json!({"title":"valid","body":"\u{001b}[2J","desktop":true,"sound":false}),
        json!({"title":"valid","body":"text","desktop":true,"sound":false,"command":"echo"}),
    ] {
        let source = format!(
            "sai.on('reply_end', function() return {{title='valid',body='text',desktop=true,sound=false}} end)\n\
             sai.on('reply_end', function() return sai.json.decode([==[{invalid}]==]) end)"
        );
        assert!(
            runtime(&source).reply_end(&event()).await.is_err(),
            "{invalid}"
        );
    }
}

/// 【展示契约测试】【无副作用结果】nil 和双开关关闭均不产生通知，声音与气泡可以独立使用。
#[tokio::test]
async fn nil_and_disabled_results_are_empty_but_sound_only_results_are_preserved() {
    let runtime = runtime(
        r#"
        sai.on('reply_end', function() return nil end)
        sai.on('reply_end', function() return {title='Disabled',body='',desktop=false,sound=false} end)
        sai.on('reply_end', function() return {title='Sound',body='',desktop=false,sound=true} end)
        "#,
    );
    let result = runtime.reply_end(&event()).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].title, "Sound");
    assert!(result[0].sound);
    assert!(!result[0].desktop);
}

/// 【展示契约测试】【通知数量】大量监听器不能绕过八条通知上限。
#[tokio::test]
async fn presentation_has_a_notification_count_limit() {
    let runtime = runtime(
        "for i=1,9 do sai.on('reply_end', function() return {title='Notice',body='',desktop=true,sound=false} end) end",
    );
    assert!(runtime
        .reply_end(&event())
        .await
        .unwrap_err()
        .to_string()
        .contains("count exceeds limit"));
}

/// 【展示契约测试】【计算预算】原清单的长时间调查预算不能延长通知计算。
#[tokio::test]
async fn presentation_restricts_the_instruction_budget_and_recovers_for_the_next_event() {
    let runtime = runtime(
        "local first=true; sai.on('reply_end', function() if first then first=false; while true do end end; return nil end)",
    );
    let error = format!("{:#}", runtime.reply_end(&event()).await.unwrap_err());
    assert!(
        error.contains("budget exceeded") || error.contains("timed out"),
        "{error}"
    );
    assert!(runtime.reply_end(&event()).await.unwrap().is_empty());
}

/// 【展示契约测试】【事件校验】拒绝任意载荷、CLI 交互面、未知状态与非法区域文本。
#[test]
fn presentation_input_is_limited_to_surface_status_and_locale() {
    for invalid in [
        json!({"surface":"cli","status":"completed","locale":"en-US"}),
        json!({"surface":"web","status":"running","locale":"en-US"}),
        json!({"surface":"web","status":"completed","locale":"en-US","workdir":"/"}),
    ] {
        assert!(serde_json::from_value::<ReplyPresentation>(invalid).is_err());
    }
    for locale in ["".into(), "x".repeat(33), "en-US\nsecret".into()] {
        assert!(ReplyPresentation { locale, ..event() }.validate().is_err());
    }
}
