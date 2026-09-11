use crate::paths::SaiPaths;
use crate::plugins::private::PrivatePluginHost;
use sai_plugin_runtime::{
    host::PluginHost, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::{path::Path, sync::Arc};

pub(super) const SOURCE: &str = r#"
    --- 【条件文件测试】【真实输出】在同一正式宿主上调用普通或条件写入
    --- @param args table 输出路径、正文和条件
    --- @return boolean|table 条件结果或普通写入元数据
    local function run(args)
        local data=sai.binary.from_bytes(args.data or "new")
        if args.ordinary then return data:write(args.path) end
        return data:write_if(args.path,args.expected)
    end
    sai.register_tool({name="run",description="Conditional file fixture",access="optional_writes",parameters={type="object"},execute=run})
"#;

/// 【条件文件测试】【基础实例】在临时应用目录下创建真实文件宿主
/// @param root 临时目录；read_paths 为读取范围；write_paths 为输出范围
/// @returns 使用同一声明与授权的运行时
pub(super) fn runtime(root: &Path, read_paths: &[&str], write_paths: &[&str]) -> PluginRuntime {
    configured(root, read_paths, write_paths, |_| {})
}

/// 【条件文件测试】【资源配置】允许调整比较预算和调用时限
/// @param root 临时目录；read_paths 为读取授权；write_paths 为输出授权；change 为清单调整
/// @returns 真实正式宿主的独立实例
pub(super) fn configured(
    root: &Path,
    read_paths: &[&str],
    write_paths: &[&str],
    change: impl FnOnce(&mut PluginManifest),
) -> PluginRuntime {
    with_host(
        Arc::new(PrivatePluginHost::new(
            &SaiPaths::for_tests(root),
            "conditional-files",
        )),
        read_paths,
        write_paths,
        SOURCE,
        change,
    )
}

/// 【条件文件测试】【宿主替换】用同一包契约检查真实宿主的异步调度和取消
/// @param host 待测宿主；read_paths 为读取范围；write_paths 为输出范围；source 为脚本；change 为清单调整
/// @returns 可执行实例
pub(super) fn with_host(
    host: Arc<dyn PluginHost>,
    read_paths: &[&str],
    write_paths: &[&str],
    source: &str,
    change: impl FnOnce(&mut PluginManifest),
) -> PluginRuntime {
    let mut manifest=PluginManifest::parse(&json!({
        "api_version":1,"id":"conditional-files","version":"1.0.0","name":"Conditional files",
        "description":"Real file conditions","entry":"init.lua",
        "capabilities":{"system":{"read_paths":read_paths},"binary":{"write_paths":write_paths}},
    }).to_string()).unwrap();
    change(&mut manifest);
    let grants = manifest.capabilities.clone();
    let package =
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap();
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}

/// 【条件文件测试】【工作目录】使用可信绝对临时目录执行有写入权限的工具
/// @param root 临时工作目录
/// @returns 可写调用上下文
pub(super) fn context(root: &Path) -> InvocationContext {
    InvocationContext {
        workdir: root.to_string_lossy().into_owned(),
        allow_writes: true,
        ..Default::default()
    }
}

/// 【条件文件测试】【独立摘要】在测试侧计算原始数据的完整 SHA-256
/// @param data 原始文件字节
/// @returns 小写十六进制摘要
pub(super) fn digest(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}
