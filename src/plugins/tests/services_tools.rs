use super::services_support::{register_service, service_descriptor};
use super::support::FixtureHost;
use crate::permission::{PermissionProfile, PermissionProfileMode};
use crate::plugins::registry::register_descriptor;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

/// 【插件测试】【组合授权】调用方只能调用其依赖，依赖可使用自己的授权；Agent 白名单仍限制全链路。
#[tokio::test]
async fn composed_plugins_use_their_own_grants_within_agent_whitelist() {
    let count = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    let calls = count.clone();
    registry.register(ToolSpec::new(
        "leaf",
        "Read leaf",
        json!({"type":"object"}),
        move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Ok("leaf-result".into()) }
        },
    ));
    register_service(
        &mut registry,
        "middle",
        r#"
        sai.register_tool({name='run',description='Read dependency',parameters={type='object'},
            execute=function() return sai.tools.call('leaf', {}) end})
    "#,
        json!({"tools":["leaf"]}),
    );
    register_service(
        &mut registry,
        "outer",
        r#"
        sai.register_tool({name='run',description='Compose dependencies',parameters={type='object'},execute=function()
            local direct, reason=pcall(sai.tools.call, 'leaf', {})
            return {direct=direct, reason=tostring(reason), catalog=sai.tools.list(),
                nested=sai.tools.call('lua__middle__run', {})}
        end})
    "#,
        json!({"tools":["lua__middle__run"]}),
    );
    let result: Value =
        serde_json::from_str(&registry.call("lua__outer__run", "{}").await.unwrap()).unwrap();
    assert_eq!(result["direct"], false);
    assert!(result["reason"]
        .as_str()
        .unwrap()
        .contains("capability is not allowed"));
    assert_eq!(result["nested"], "leaf-result");
    assert_eq!(result["catalog"].as_array().unwrap().len(), 1);
    assert_eq!(result["catalog"][0]["name"], "lua__middle__run");
    let filtered = registry.clone_filtered(&["lua__outer__run", "lua__middle__run"]);
    let error = filtered.call("lua__outer__run", "{}").await.unwrap_err();
    assert!(
        format!("{error:#}").contains("plugin tool is not available: leaf"),
        "{error:#}"
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

/// 【插件测试】【循环依赖】A 调用 B 后，B 不能再次进入 A；目录和调用入口同时收窄。
#[tokio::test]
async fn cyclic_plugin_dependencies_fail_without_waiting_for_a_vm_lock() {
    let mut registry = ToolRegistry::new();
    for (id, dependency) in [("first", "second"), ("second", "first")] {
        register_service(
            &mut registry,
            id,
            &format!(
                r#"
            sai.register_tool({{name='run',description='Nested call',parameters={{type='object'}},
                execute=function() return sai.tools.call('lua__{dependency}__run', {{}}) end}})
        "#
            ),
            json!({"tools":[format!("lua__{dependency}__run")]}),
        );
    }
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        registry.call("lua__first__run", "{}"),
    )
    .await
    .expect("recursive call waited for a VM lock");
    let error = result.unwrap_err();
    assert!(
        format!("{error:#}").contains("plugin tool is not available: lua__first__run"),
        "{error:#}"
    );
}

/// 【插件测试】【原生包装递归】捕获旧工具表的原生包装器也必须在事件分发前拒绝递归。
#[tokio::test]
async fn native_wrapper_recursion_is_rejected_before_observer_dispatch() {
    let holder = Arc::new(Mutex::new(Weak::<ToolRegistry>::new()));
    let capture = holder.clone();
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "wrapper",
        "Call captured registry",
        json!({"type":"object"}),
        move |_| {
            let registry = capture.lock().unwrap().upgrade().unwrap();
            async move { registry.call("lua__loop__run", "{}").await }
        },
    ));
    let mut plugin = service_descriptor(
        "loop",
        r#"
        sai.on('tool_call', function() end)
        sai.on('tool_result', function() end)
        sai.register_tool({name='run',description='Recursive wrapper',parameters={type='object'},
            execute=function() return sai.tools.call('wrapper', {}) end})
    "#,
        json!({"tools":["wrapper"]}),
    );
    plugin.package.manifest.limits.timeout_ms = 300;
    register_descriptor(
        &mut registry,
        plugin,
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    let registry = Arc::new(registry);
    *holder.lock().unwrap() = Arc::downgrade(&registry);
    let error = registry.call("lua__loop__run", "{}").await.unwrap_err();
    assert!(
        format!("{error:#}").contains("recursive plugin invocation"),
        "{error:#}"
    );
}

/// 【插件测试】【旧表观察者】原生包装器使用旧注册表执行其他工具时，也不能重入调用方的监听器 VM。
#[tokio::test]
async fn native_wrapper_with_old_registry_keeps_nonrecursive_observers_safe() {
    let holder = Arc::new(Mutex::new(Weak::<ToolRegistry>::new()));
    let capture = holder.clone();
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "leaf",
        "Read captured registry",
        json!({"type":"object"}),
        |_| async { Ok("leaf-result".into()) },
    ));
    registry.register(ToolSpec::new(
        "wrapper",
        "Call captured registry",
        json!({"type":"object"}),
        move |_| {
            let registry = capture.lock().unwrap().upgrade().unwrap();
            async move { registry.call("leaf", "{}").await }
        },
    ));
    let mut plugin = service_descriptor(
        "observer",
        r#"
        sai.on('tool_call', function() end)
        sai.on('tool_result', function() end)
        sai.register_tool({name='run',description='Nonrecursive wrapper',parameters={type='object'},
            execute=function() return sai.tools.call('wrapper', {}) end})
    "#,
        json!({"tools":["wrapper"]}),
    );
    plugin.package.manifest.limits.timeout_ms = 300;
    register_descriptor(
        &mut registry,
        plugin,
        Arc::new(FixtureHost::default()),
        false,
    )
    .unwrap();
    let registry = Arc::new(registry);
    *holder.lock().unwrap() = Arc::downgrade(&registry);
    assert_eq!(
        registry.call("lua__observer__run", "{}").await.unwrap(),
        "leaf-result"
    );
}

/// 【插件测试】【检查隔离】正在执行的插件同时监听工具事件，嵌套检查不能重入其 VM 或产生被拒绝工具的副作用。
#[tokio::test]
async fn nested_observers_run_on_independent_vms_and_denials_prevent_side_effects() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "leaf",
        "Count calls",
        json!({"type":"object"}),
        move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            async { Ok("called".into()) }
        },
    ));
    register_service(
        &mut registry,
        "observer",
        r#"
        sai.on('tool_call', function(event)
            if event.name == 'leaf' then return {deny='fixture policy'} end
        end)
        sai.on('tool_result', function() end)
        sai.register_tool({name='run',description='Checked dependency',parameters={type='object'},
            execute=function() return sai.tools.call('leaf', {}) end})
    "#,
        json!({"tools":["leaf"]}),
    );
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        registry.call("lua__observer__run", "{}"),
    )
    .await
    .expect("observer reentered the active VM")
    .unwrap_err();
    assert!(format!("{error:#}").contains("denied leaf: fixture policy"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

/// 【插件测试】【权限热切换】调用开始后切换到计划模式，后续嵌套写入仍由实时权限阻止。
#[tokio::test]
async fn live_plan_mode_blocks_writes_after_plugin_invocation_started() {
    let profile = PermissionProfile::new(
        PermissionProfileMode::Yolo,
        std::env::current_dir().unwrap(),
        None,
    );
    let switch = profile.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let writes = count.clone();
    let mut registry = ToolRegistry::new();
    registry.set_permission_profile(profile);
    registry.register(ToolSpec::new(
        "switch_mode",
        "Switch to plan",
        json!({"type":"object"}),
        move |_| {
            switch.set_mode(PermissionProfileMode::Plan);
            async { Ok("plan".into()) }
        },
    ));
    registry.register(
        ToolSpec::new(
            "write_leaf",
            "Write leaf",
            json!({"type":"object"}),
            move |_| {
                writes.fetch_add(1, Ordering::SeqCst);
                async { Ok("written".into()) }
            },
        )
        .writes(),
    );
    register_service(
        &mut registry,
        "writer",
        r#"
        sai.register_tool({name='run',description='Write after switch',access='writes',parameters={type='object'},execute=function()
            sai.tools.call('switch_mode', {})
            return sai.tools.call('write_leaf', {})
        end})
    "#,
        json!({"tools":["switch_mode", "write_leaf"]}),
    );
    let error = registry.call("lua__writer__run", "{}").await.unwrap_err();
    assert!(
        format!("{error:#}").to_ascii_lowercase().contains("plan"),
        "{error:#}"
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

/// 【插件测试】【服务回收】成功和失败均释放调用服务，长期存活的 Lua VM 不保留注册表闭包。
#[tokio::test]
async fn completed_invocations_release_captured_registry_references() {
    let sentinel = Arc::new(());
    let capture = sentinel.clone();
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "leaf",
        "Hold lifetime marker",
        json!({"type":"object"}),
        move |_| {
            let marker = capture.clone();
            async move {
                drop(marker);
                Ok("leaf".into())
            }
        },
    ));
    register_service(
        &mut registry,
        "lifetime",
        r#"
        sai.register_tool({name='run',description='Call and optionally fail',parameters={type='object'},execute=function(args)
            local result=sai.tools.call('leaf', {})
            if args.fail then error('fixture failure') end
            return result
        end})
    "#,
        json!({"tools":["leaf"]}),
    );
    for arguments in ["{}", r#"{"fail":true}"#, "{}"] {
        let result = registry.call("lua__lifetime__run", arguments).await;
        assert_eq!(result.is_err(), arguments.contains("true"));
        assert_eq!(Arc::strong_count(&sentinel), 2);
    }
    drop(registry);
    assert_eq!(Arc::strong_count(&sentinel), 1);
}
