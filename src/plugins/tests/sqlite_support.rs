use crate::{
    paths::SaiPaths, plugins::discovery::PluginDescriptor, plugins::private::PrivatePluginHost,
};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

pub(super) const SOURCE: &str = r#"
    --- 【数据库文件测试】【只读查询】从已授权文件取得完整镜像并返回查询结果及修订摘要
    --- @param args table 输入路径、读取上限和结构化查询
    --- @return table 完整查询与文件摘要
    local function query(args)
        local data=sai.binary.read_file(args.path,{max_bytes=args.read_bytes or 1048576})
        return {data=sai.sqlite.query(data,args.query),revision=data:sha256()}
    end
    --- 【数据库文件测试】【条件发布】在内存中完成批次，再按显式摘要条件发布
    --- @param args table 可选来源、变更、容量、输出路径及原修订
    --- @return boolean|table 条件发布结果，或未发布的镜像信息
    local function apply(args)
        local original=nil
        if args.source then original=sai.binary.read_file(args.source,{max_bytes=args.read_bytes or 1048576}) end
        local updated=sai.sqlite.apply(original,args.changes,{max_bytes=args.capacity or 65536})
        if args.path then return updated:write_if(args.path,args.expected) end
        return {bytes=updated:len(),revision=updated:sha256()}
    end
    sai.register_tool({name='query',description='Query a database snapshot',parameters={type='object'},execute=query})
    sai.register_tool({name='apply',description='Compute or publish a snapshot',access='optional_writes',parameters={type='object'},execute=apply})
    sai.register_tool({name='write',description='Publish a database snapshot',access='writes',parameters={type='object'},execute=apply})
"#;

/// 【数据库文件测试】【外部包】声明相互独立的目录读取和二进制写入能力
/// @returns 可交给实际注册、发现和授权流程的描述符
pub(super) fn descriptor() -> PluginDescriptor {
    let mut descriptor = super::support::descriptor("sqlite-files", SOURCE);
    descriptor.package.manifest.capabilities = serde_json::from_value(json!({
        "system":{"read_paths":["allowed"]},"binary":{"write_paths":["allowed"]}
    }))
    .unwrap();
    descriptor.setting.grants = Some(descriptor.package.manifest.capabilities.clone());
    descriptor
}

/// 【数据库文件测试】【正式宿主】按能力交集加载外部测试包，不添加数据库专属权限
/// @param root 隔离目录；change 为已有授权的调整
/// @returns 绑定正式文件宿主的独立 Lua 实例
pub(super) fn runtime(root: &Path, change: impl FnOnce(&mut Capabilities)) -> PluginRuntime {
    let descriptor = descriptor();
    let mut grants = descriptor.grants();
    change(&mut grants);
    PluginRuntime::load(
        descriptor.runtime_package(),
        json!({}),
        grants,
        Arc::new(
            PrivatePluginHost::for_descriptor(&SaiPaths::for_tests(root), &descriptor).unwrap(),
        ),
    )
    .unwrap()
}

/// 【数据库文件测试】【可信上下文】为每次调用提供宿主工作目录与写入许可
/// @param root 隔离工作目录；writes 为本次写入许可
/// @returns 不受 Lua 参数影响的调用上下文
pub(super) fn context(root: &Path, writes: bool) -> InvocationContext {
    InvocationContext {
        workdir: root.display().to_string(),
        allow_writes: writes,
        ..Default::default()
    }
}

/// 【数据库文件测试】【基础数据】使用原生 SQLite 创建可比较的独立普通表
/// @param root 隔离目录
/// @returns 数据库原始字节；连接已关闭，不依赖 WAL
pub(super) fn seed(root: &Path) -> Vec<u8> {
    std::fs::create_dir_all(root.join("allowed")).unwrap();
    let path = root.join("allowed/notes.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE notes(id INTEGER PRIMARY KEY,text TEXT NOT NULL); INSERT INTO notes VALUES(1,'original');").unwrap();
    drop(connection);
    std::fs::read(path).unwrap()
}

/// 【数据库文件测试】【查询参数】读取笔记全部普通字段
/// @param path 授权路径
/// @returns 通用查询工具的输入
pub(super) fn query(path: &str) -> Value {
    json!({"path":path,"query":{"table":"notes","columns":["id","text"],"order_by":[{"column":"id"}]}})
}

/// 【数据库文件测试】【变更参数】指定同一条记录的新文字，其他字段由冲突更新保留
/// @param text 本次文字
/// @returns 单项结构化 upsert 批次
pub(super) fn changes(text: &str) -> Value {
    json!([{"op":"upsert","table":"notes","key":"id","rows":[{"id":1,"text":text}]}])
}

/// 【数据库文件测试】【结果解析】执行真实运行时并保留完整错误原因
/// @param plugin 实例；root 为目录；name 为工具；args 为输入；writes 为写入许可
/// @returns JSON 结果或宿主错误
pub(super) async fn call(
    plugin: &PluginRuntime,
    root: &Path,
    name: &str,
    args: Value,
    writes: bool,
) -> anyhow::Result<Value> {
    Ok(serde_json::from_str(
        &plugin.call_tool(name, args, context(root, writes)).await?,
    )?)
}
