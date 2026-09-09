use super::support::FixtureHost;
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【诊断对照测试】【纯函数入口】仅追加测试工具，所有业务函数来自正式发布的 Lua 包。
/// @returns 可执行固定对照样本的运行时
fn runtime() -> PluginRuntime {
    let original = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "diagnostic-evidence")
        .unwrap();
    let mut sources = original.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(r#"
local text = require('text')
local arguments = require('arguments')
local classify = require('input_method.classify')
local paths = require('input_method.paths')
local processes = require('input_method.processes')
local modules = require('input_method.modules')
local areas = require('areas')
local packages = require('packages')
local collect = require('collect')
local environment = require('input_method.environment')
--- 【诊断对照测试】【样本解释】按固定输入调用原模块，无宿主系统操作
--- @param input table 固定样本
--- @return table 实际结果
local function reference(input)
    if input.kind == 'args' then return arguments.parse(input.args) end
    if input.kind == 'classify' then
        local raw = classify.toolkit(input.text)
        local display = classify.display(input.text, input.env, input.socket, input.loaded or {})
        local toolkit = classify.refine(raw, display)
        return {raw=raw, display=display, toolkit=toolkit, paths=classify.relevant_paths(toolkit)}
    end
    if input.kind == 'path' then
        return paths.evaluate({toolkit=input.toolkit, display_mode=input.display, runtime_observed=input.runtime,
            target_env=input.env, loaded_input_modules=input.loaded or {}, available_input_modules=input.available or {},
            immodule_cache=input.cache or {}, wayland_protocol={compositor_supports_text_input_v3=input.compositor==true,
                fcitx5_wayland_frontend_loaded=input.frontend==true},
            locale_info={target_lang=input.lang or sai.json.null,target_lc_ctype=input.ctype or sai.json.null,locale_valid=input.locale_valid==true}})
    end
    if input.kind == 'text' then
        return {clip=text.clip(input.text,input.limit), masked=areas.mask_addresses(input.text),
            owner=packages.owner_name(input.text) or sai.json.null,package_line=packages.relevant_line(input.text),
            gpu=areas.gpu_blocks(input.text,input.home),os=collect.os_release_value(input.text,'PRETTY_NAME') or sai.json.null,
            safe=text.safe_command_name(input.text),module_path=processes.module_path(input.text,input.home) or sai.json.null,
            module_file=modules.is_module_file(input.text:lower()),redacted=text.redact(input.text,input.home)}
    end
    if input.kind == 'process' then
        return {matches=processes.matches(input.text,input.target,input.own_pid),mentions=modules.mentions_target(input.text,input.target)}
    end
    if input.kind == 'sockets' then
        local mode, facts=environment.socket_mode(input.socket_text,input.unix_text,input.pids)
        return {mode=mode,facts=facts}
    end
    error('unknown diagnostic reference kind')
end
sai.register_tool({name='reference',description='Compare diagnostic pure functions',parameters={type='object'},execute=reference})
"#);
    let grants = original.manifest.capabilities.clone();
    PluginRuntime::load(
        PluginPackage::new(original.manifest, sources).unwrap(),
        json!({}),
        grants,
        Arc::new(FixtureHost::default()),
    )
    .unwrap()
}

/// 【诊断对照测试】【原版行为】验证参数、文本、进程过滤、工具包分类和路径状态的固定对照。
#[tokio::test]
async fn lua_diagnostic_rules_match_original_rust_fixtures() {
    let runtime = runtime();
    let mut total = 0;
    for source in [
        include_str!("fixtures/diagnostic-evidence/arguments.json"),
        include_str!("fixtures/diagnostic-evidence/classifiers.json"),
        include_str!("fixtures/diagnostic-evidence/paths.json"),
        include_str!("fixtures/diagnostic-evidence/text.json"),
        include_str!("fixtures/diagnostic-evidence/processes.json"),
    ] {
        let fixtures: Value = serde_json::from_str(source).unwrap();
        assert_eq!(
            fixtures["source_commit"],
            "a1bb8e40f87b0898377cfcded6d4d0cc76f01d3e"
        );
        for case in fixtures["cases"].as_array().unwrap() {
            let output = runtime
                .call_tool(
                    "reference",
                    case["input"].clone(),
                    InvocationContext::default(),
                )
                .await;
            if let Some(error) = case["output"]["error"].as_str() {
                assert!(
                    format!("{:#}", output.unwrap_err()).contains(error),
                    "{}",
                    case["name"]
                );
            } else {
                let output: Value = serde_json::from_str(&output.unwrap()).unwrap();
                assert_eq!(output, case["output"], "{}", case["name"]);
            }
            total += 1;
        }
    }
    assert_eq!(total, 276);
}

/// 【诊断对照测试】【套接字修复】使用正确 inode 列，PID 12 不得误匹配 PID 123。
#[tokio::test]
async fn socket_evidence_uses_the_inode_column_and_complete_process_ids() {
    let runtime = runtime();
    let unix_text="Num RefCount Protocol Flags Type St Inode Path\n000: 2 0 0 0001 01 901 /tmp/.X11-unix/X0\n001: 2 0 0 0001 01 902 /run/user/1000/wayland-0\n";
    for (socket_text, pids, mode) in [
        (
            "u_str ESTAB 0 0 * 901 * 910 users:((app,pid=123,fd=3))",
            vec![12],
            "unknown",
        ),
        (
            "u_str ESTAB 0 0 * 901 * 910 users:((app,pid=123,fd=3))",
            vec![123],
            "x_wayland",
        ),
        (
            "u_str ESTAB 0 0 * 902 * 910 users:((app,pid=123,fd=3))",
            vec![123],
            "wayland_native",
        ),
    ] {
        let output=runtime.call_tool("reference",json!({"kind":"sockets","socket_text":socket_text,"unix_text":unix_text,"pids":pids}),InvocationContext::default()).await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap()["mode"],
            mode
        );
    }
}
