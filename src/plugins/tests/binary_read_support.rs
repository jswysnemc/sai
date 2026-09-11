use crate::paths::SaiPaths;
use crate::plugins::private::PrivatePluginHost;
use sai_plugin_runtime::{InvocationContext, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

pub(super) const SOURCE: &str = r#"
    --- 【二进制读取测试】【真实读取】读取完整原始字节，返回便于核验的摘要和小段字节
    --- @param args table 文件路径及读取选项
    --- @return table 缓冲长度、摘要及原始字节值
    local function read(args)
        local data = sai.binary.read_file(args.path, args.options)
        local first = data:bytes(0, math.min(data:len(), 16))
        local bytes = sai.json.array()
        for index = 1, #first do bytes[index] = first:byte(index) end
        local result = {length=data:len(),sha256=data:sha256(),bytes=bytes}
        data:close()
        return result
    end
    sai.register_tool({name="read",description="Read raw file bytes",parameters={type="object"},execute=read})
"#;

/// 【二进制读取测试】【真实运行时】为临时目录构造受目录授权限制的实际宿主
/// @param root 隔离应用目录；read_paths 为读取范围
/// @returns 包声明和授权一致的独立运行时
pub(super) fn runtime(root: &std::path::Path, read_paths: &[&str]) -> PluginRuntime {
    configured(root, read_paths, SOURCE, |_| {})
}

/// 【二进制读取测试】【场景配置】调整测试源码、资源限制和独立写入授权
/// @param root 隔离目录；read_paths 为读取范围；source 为入口；configure 为清单调整
/// @returns 使用真实私有宿主的运行时
pub(super) fn configured(
    root: &std::path::Path,
    read_paths: &[&str],
    source: &str,
    configure: impl FnOnce(&mut PluginManifest),
) -> PluginRuntime {
    let mut manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"binary-read","version":"1.0.0","name":"Binary read",
            "description":"Read contract fixture","entry":"init.lua",
            "capabilities":{"system":{"read_paths":read_paths}},
        })
        .to_string(),
    )
    .unwrap();
    configure(&mut manifest);
    let grants = manifest.capabilities.clone();
    let package =
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap();
    PluginRuntime::load(
        package,
        json!({}),
        grants,
        Arc::new(PrivatePluginHost::new(
            &SaiPaths::for_tests(root),
            "binary-read",
        )),
    )
    .unwrap()
}

/// 【二进制读取测试】【调用上下文】绑定真实绝对目录，不授予写入能力
/// @param root 测试工作目录
/// @returns 只读工具上下文
pub(super) fn context(root: &std::path::Path) -> InvocationContext {
    InvocationContext {
        workdir: root.display().to_string(),
        ..Default::default()
    }
}
