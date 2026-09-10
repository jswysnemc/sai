mod common;
#[path = "binary/support.rs"]
mod support;

use sai_plugin_runtime::{PluginRuntime, ToolAccess};
use serde_json::{json, Value};
use std::sync::Arc;
use support::*;

/// 【权限测试】【可选写入】同一工具在只读上下文中查询，修改 Lua 上下文不能取得写入权限。
/// @returns 无；宿主记录只包含真实获准的写入
#[tokio::test]
async fn optional_writes_follow_trusted_host_permission() {
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Optional write',access='optional_writes',parameters={type='object'},execute=function(args,ctx)
            local allowed=ctx.allow_writes
            ctx.allow_writes=true
            ctx.workdir='/forged'
            local b=sai.binary.decode_base64('YWJj')
            local ok,value=pcall(b.write,b,'output/result')
            return {allowed=allowed,written=ok,value=ok and value or tostring(value)}
        end})
        "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    assert_eq!(plugin.tools()[0].access, ToolAccess::OptionalWrites);
    for writable in [false, true, false] {
        let result: Value = serde_json::from_str(
            &plugin
                .call_tool("run", json!({}), context(writable))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["allowed"], writable);
        assert_eq!(result["written"], writable);
    }
    let writes = host.writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].2.workdir, "/trusted");
    assert!(writes[0].2.allow_writes);
}

/// 【权限测试】【命令契约】可选写入仅适用于工具，命令声明不能绕过原有权限规则。
/// @returns 无；非法包在注册阶段明确失败
#[test]
fn commands_reject_optional_writes() {
    let package = common::package("sai.register_command({name='run',description='Invalid command',access='optional_writes',execute=function() end})");
    let error = PluginRuntime::load(
        package,
        json!({}),
        Default::default(),
        Arc::new(Host::default()),
    )
    .err()
    .unwrap();
    assert!(format!("{error:#}").contains("optional_writes is only supported for tools"));
}
