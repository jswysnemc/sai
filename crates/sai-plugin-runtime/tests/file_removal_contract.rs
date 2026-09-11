mod common;
#[path = "file_removal/support.rs"]
mod support;

use sai_plugin_runtime::InvocationContext;
use serde_json::json;

/// 【文件删除测试】【接口存在】两种删除行为必须分别安装，并在没有授权时拒绝访问
/// @returns 无；拒绝结果来自真实接口而非缺失函数
#[tokio::test]
async fn file_removal_apis_exist_and_require_explicit_grants() {
    let plugin = common::runtime(
        r#"
        --- 【文件删除测试】【权限探测】调用两个独立文件接口
        --- @return string 拒绝结果
        local function run()
            for _,name in ipairs({"remove_file","trash_file"}) do
                local operation=sai.fs[name]
                assert(type(operation)=="function",name.." API is missing")
                local ok,message=pcall(operation,"output/a")
                assert(not ok and tostring(message):find("not allowed",1,true),tostring(message))
            end
            return "denied"
        end
        sai.register_tool({name="run",description="File removal grants",parameters={type="object"},execute=run})
        "#,
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "denied"
    );
}

/// 【文件删除测试】【旧宿主兼容】未实现新接口的宿主必须明确拒绝两种操作
/// @returns 无；不能隐式调用旧工具或把回收站降级为永久删除
#[tokio::test]
async fn file_removal_is_unavailable_in_hosts_without_an_explicit_implementation() {
    let plugin = support::runtime(
        support::SOURCE,
        std::sync::Arc::new(common::RecordingHost::default()),
        |_| {},
    );
    for operation in ["remove_file", "trash_file"] {
        let error = plugin
            .call_tool(
                "run",
                json!({"operation":operation}),
                support::context(true),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("file removal is unavailable in this host"),
            "{error:#}"
        );
    }
}
