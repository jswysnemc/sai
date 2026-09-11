use super::{read_authorized, read_chunks};
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{
    host::*, Capabilities, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Copy)]
enum Behavior {
    Plain,
    Grow(usize),
    Interrupted,
    CancelAfterChunk,
    CancelAtEof,
    CancelBeforeOpen,
}

struct ChangingFile<'a> {
    file: std::fs::File,
    path: &'a std::path::Path,
    behavior: Behavior,
    first: bool,
    cancelled: &'a AtomicBool,
}

impl Read for ChangingFile<'_> {
    /// 【二进制读取测试】【确定性文件变化】在实际文件读取边界追加字节、模拟中断或设置取消
    /// @param bytes 生产读取循环交付的有界输出切片
    /// @returns 实际文件字节数，或一次可重试的系统中断
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let first = std::mem::replace(&mut self.first, false);
        if first && matches!(self.behavior, Behavior::Interrupted) {
            return Err(ErrorKind::Interrupted.into());
        }
        let count = self.file.read(bytes)?;
        if first {
            if let Behavior::Grow(size) = self.behavior {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(self.path)?
                    .write_all(&vec![255; size])?;
            }
        }
        if (first && matches!(self.behavior, Behavior::CancelAfterChunk))
            || (count == 0 && matches!(self.behavior, Behavior::CancelAtEof))
        {
            self.cancelled.store(true, Ordering::Release);
        }
        Ok(count)
    }
}

struct Host {
    path: PathBuf,
    behavior: Mutex<Behavior>,
}

#[async_trait]
impl PluginHost for Host {
    /// 【二进制读取测试】【网络隔离】文件循环测试不访问网络
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 固定错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【二进制读取测试】【生产循环】将真实运行时预留缓冲交给生产分块读取函数
    /// @param path 路径；buffer 为预算；context 为可信目录；capabilities 为授权
    /// @returns 生产读取函数的完整结果
    async fn read_binary(
        &self,
        path: String,
        buffer: BinaryReadBuffer,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryData> {
        let behavior = *self.behavior.lock().unwrap();
        let cancelled = AtomicBool::new(matches!(behavior, Behavior::CancelBeforeOpen));
        if matches!(behavior, Behavior::CancelBeforeOpen) {
            return read_authorized(&path, buffer, &context, &capabilities, &cancelled);
        }
        let mut reader = ChangingFile {
            file: std::fs::File::open(&self.path)?,
            path: &self.path,
            behavior,
            first: true,
            cancelled: &cancelled,
        };
        read_chunks(&mut reader, buffer, &cancelled)
    }
}

/// 【二进制读取测试】【循环运行时】为生产读取循环创建一 KiB 总预算的实际 Lua 实例
/// @param host 可控制文件变化的宿主
/// @returns 具有普通读取能力的独立运行时
fn runtime(host: Arc<Host>) -> PluginRuntime {
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"read-stream","version":"1.0.0","name":"Read stream",
            "description":"Deterministic read loop","entry":"init.lua",
            "capabilities":{"system":{"read_paths":["."]}},"limits":{"binary_bytes":1024},
        })
        .to_string(),
    )
    .unwrap();
    let grants = manifest.capabilities.clone();
    let source = r#"
        --- 【二进制读取测试】【循环调用】由真实运行时提供预留缓冲
        --- @param args table 可选测试路径
        --- @return integer 完整字节数
        local function read(args)
            return sai.binary.read_file(args.path or "image.bin"):len()
        end
        sai.register_tool({name="read",description="Read loop",parameters={type="object"},execute=read})
    "#;
    let package =
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap();
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}

/// 【二进制读取测试】【文件增长】初始长度检查之后新增的字节也必须计入完整读取上限
/// @returns 无；达到上限时完整成功，再增加一字节时失败且预算可恢复
#[tokio::test]
async fn growing_files_are_checked_until_eof_instead_of_trusting_initial_length() {
    for added in [512, 513] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.bin");
        std::fs::write(&path, vec![0; 512]).unwrap();
        let host = Arc::new(Host {
            path: path.clone(),
            behavior: Mutex::new(Behavior::Grow(added)),
        });
        let plugin = runtime(host.clone());
        let result = plugin
            .call_tool("read", json!({}), InvocationContext::default())
            .await;
        if added == 512 {
            assert_eq!(result.unwrap(), "1024");
        } else {
            let error = result.unwrap_err();
            assert!(format!("{error:#}").contains("size limit"), "{error:#}");
        }
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            (512 + added) as u64
        );
        *host.behavior.lock().unwrap() = Behavior::Plain;
        std::fs::write(&path, vec![0; 512]).unwrap();
        assert_eq!(
            plugin
                .call_tool("read", json!({}), InvocationContext::default())
                .await
                .unwrap(),
            "512"
        );
    }
}

/// 【二进制读取测试】【系统中断】可重试的系统调用中断不能造成截断结果或永久失败
/// @returns 无；生产读取循环继续直到完整 EOF
#[tokio::test]
async fn interrupted_file_reads_retry_without_losing_bytes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("image.bin");
    std::fs::write(&path, vec![255; 512]).unwrap();
    let plugin = runtime(Arc::new(Host {
        path,
        behavior: Mutex::new(Behavior::Interrupted),
    }));
    assert_eq!(
        plugin
            .call_tool("read", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "512"
    );
}

/// 【二进制读取测试】【取消边界】授权前、分块之后和最终 EOF 时取消都不能生成成功缓冲
/// @returns 无；每次拒绝之后同一实例仍可读满原预算
#[tokio::test]
async fn cancellation_before_open_during_read_and_at_eof_rejects_the_result() {
    for behavior in [
        Behavior::CancelBeforeOpen,
        Behavior::CancelAfterChunk,
        Behavior::CancelAtEof,
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("image.bin");
        std::fs::write(&path, vec![0; 1024]).unwrap();
        let host = Arc::new(Host {
            path,
            behavior: Mutex::new(behavior),
        });
        let plugin = runtime(host.clone());
        let error = plugin
            .call_tool(
                "read",
                json!({"path":"missing.bin"}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("binary read cancelled"),
            "{error:#}"
        );
        *host.behavior.lock().unwrap() = Behavior::Plain;
        assert_eq!(
            plugin
                .call_tool("read", json!({}), InvocationContext::default())
                .await
                .unwrap(),
            "1024"
        );
    }
}
