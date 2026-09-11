use super::*;
use anyhow::Result;
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::*, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::sync::Mutex;

#[derive(Default)]
struct Capture(Mutex<Option<BinaryData>>);

#[async_trait]
impl PluginHost for Capture {
    /// 【暂存发布测试】【网络隔离】创建预算数据不访问网络
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        anyhow::bail!("unexpected HTTP")
    }

    /// 【暂存发布测试】【租约保留】通过真实运行时获取已经计费的原始数据
    /// @param request 条件；data 为受控字节；context 为目录；capabilities 为授权
    /// @returns 固定条件失败，测试侧继续持有数据租约
    async fn write_binary_if(
        &self,
        _: BinaryConditionalWrite,
        data: BinaryData,
        _: SystemContext,
        _: Capabilities,
    ) -> Result<bool> {
        *self.0.lock().unwrap() = Some(data);
        Ok(false)
    }
}

/// 【暂存发布测试】【受控输入】在真实 Lua 回调内生成缓冲，保持预算构造边界
/// @returns 待写入数据，不绕过生产预算类型
async fn input() -> BinaryData {
    let host = Arc::new(Capture::default());
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"stage-test","version":"1.0.0","name":"Staging tests",
            "description":"Staging lifecycle","entry":"init.lua",
            "capabilities":{"system":{"read_paths":["."]},"binary":{"write_paths":["."]}},
        })
        .to_string(),
    )
    .unwrap();
    let grants = manifest.capabilities.clone();
    let source = r#"
        --- 【暂存发布测试】【缓冲生成】把受控数据交给测试宿主
        --- @return boolean 固定条件失败
        local function run() return sai.binary.from_bytes("new"):write_if("a",nil) end
        sai.register_tool({name="run",description="Capture buffer",access="writes",parameters={type="object"},execute=run})
    "#;
    let package =
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap();
    let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    plugin
        .call_tool(
            "run",
            json!({}),
            InvocationContext {
                allow_writes: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let data = host.0.lock().unwrap().take().unwrap();
    data
}

/// 【暂存发布测试】【输出规划】只给测试目录下的 output 授权，取得实际父目录
/// @param root 临时根目录
/// @returns 实际写入目的地
fn destination(root: &std::path::Path) -> paths::Destination {
    let context = SystemContext {
        workdir: root.to_str().unwrap().into(),
        allow_writes: true,
    };
    let capabilities =
        serde_json::from_value(json!({"binary":{"write_paths":["output"]}})).unwrap();
    paths::plan("output/file", &context, &capabilities)
        .unwrap()
        .create()
        .unwrap()
}

/// 【暂存发布测试】【等待发布的锁】完整暂存不立即覆盖目标，暂存清理结束后才释放锁
/// @returns 无；取消或丢弃结果保留旧文件并删除临时文件
#[tokio::test]
async fn dropping_completed_staging_preserves_old_bytes_and_releases_the_lock_after_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let destination = destination(root.path());
    std::fs::write(root.path().join("output/file"), b"old").unwrap();
    let state = root.path().join("state");
    let cancelled = AtomicBool::new(false);
    let lock = file_lock::acquire(&state, &cancelled).unwrap();
    let stage = prepare(destination, input().await, Some(lock), &cancelled).unwrap();
    let contender = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(state.join("plugin-binary-write.lock"))
        .unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    assert_eq!(
        std::fs::read(root.path().join("output/file")).unwrap(),
        b"old"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("output"))
            .unwrap()
            .count(),
        2
    );
    drop(stage);
    contender.try_lock().unwrap();
    assert_eq!(
        std::fs::read(root.path().join("output/file")).unwrap(),
        b"old"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("output"))
            .unwrap()
            .count(),
        1
    );
}

/// 【暂存发布测试】【发布失败回收】目录替换导致发布失败时清理暂存并归还互斥
/// @returns 无；真实目录保留且没有暂存文件残留
#[tokio::test]
async fn failed_publication_cleans_staging_and_unlocks_the_writer() {
    let root = tempfile::tempdir().unwrap();
    let destination = destination(root.path());
    let state = root.path().join("state");
    let cancelled = AtomicBool::new(false);
    let lock = file_lock::acquire(&state, &cancelled).unwrap();
    let stage = prepare(destination, input().await, Some(lock), &cancelled).unwrap();
    std::fs::create_dir(root.path().join("output/file")).unwrap();
    assert!(publish(&stage).is_err());
    drop(stage);
    let _next = file_lock::acquire(&state, &cancelled).unwrap();
    assert!(root.path().join("output/file").is_dir());
    assert_eq!(
        std::fs::read_dir(root.path().join("output"))
            .unwrap()
            .count(),
        1
    );
}
