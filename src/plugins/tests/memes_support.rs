use crate::{config::AppConfig, paths::SaiPaths, plugins::private::PrivatePluginHost};
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::{host::*, Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

pub(super) struct MemeHost {
    inner: PrivatePluginHost,
    pub displays: Mutex<Vec<(String, Option<String>)>>,
    pub fail_display: AtomicBool,
    pub fail_removal: AtomicBool,
    pub pause_after_removal: AtomicBool,
    pub removed: tokio::sync::Notify,
    pub pause_after_image: AtomicBool,
    pub image_created: tokio::sync::Notify,
}

impl MemeHost {
    /// 【表情测试】【真实文件宿主】隔离应用目录，只替代终端绘制和可控故障点
    /// @param root 临时应用根目录
    /// @returns 可共享的正式文件宿主代理
    pub fn new(root: &Path) -> Arc<Self> {
        Arc::new(Self {
            inner: PrivatePluginHost::new(&SaiPaths::for_tests(root), "memes"),
            displays: Mutex::new(Vec::new()),
            fail_display: AtomicBool::new(false),
            fail_removal: AtomicBool::new(false),
            pause_after_removal: AtomicBool::new(false),
            removed: tokio::sync::Notify::new(),
            pause_after_image: AtomicBool::new(false),
            image_created: tokio::sync::Notify::new(),
        })
    }
}

#[async_trait]
impl PluginHost for MemeHost {
    /// 【表情测试】【网络隔离】表情业务不使用直接网络请求
    /// @param request 请求；capabilities 为权限；allow_writes 为宿主写入许可
    /// @returns 明确错误
    async fn http(&self, _: HttpRequest, _: Capabilities, _: bool) -> Result<HttpResponse> {
        bail!("unexpected HTTP")
    }

    /// 【表情测试】【文件元数据】保留实际读取授权和路径解析
    /// @param path 路径；context 为上下文；capabilities 为授权
    /// @returns 实际文件信息
    async fn file_info(
        &self,
        path: String,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<Option<FileInfo>> {
        self.inner.file_info(path, context, capabilities).await
    }

    /// 【表情测试】【路径解析】复用正式宿主的越界和符号链接检查
    /// @param path 输入路径；context 为目录；capabilities 为授权
    /// @returns 规范绝对路径
    async fn real_path(
        &self,
        path: String,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<String> {
        self.inner.real_path(path, context, capabilities).await
    }

    /// 【表情测试】【完整读取】通过正式宿主取得有界文件快照
    /// @param path 路径；buffer 为预算；context 为目录；capabilities 为授权
    /// @returns 实际字节缓冲
    async fn read_binary(
        &self,
        path: String,
        buffer: BinaryReadBuffer,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<BinaryData> {
        self.inner
            .read_binary(path, buffer, context, capabilities)
            .await
    }

    /// 【表情测试】【条件输出】实际索引与图片写入共用正式文件锁
    /// @param request 路径和条件；data 为字节；context 为目录；capabilities 为授权
    /// @returns 是否实际发布
    async fn write_binary_if(
        &self,
        request: BinaryConditionalWrite,
        data: BinaryData,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<bool> {
        let image = Path::new(&request.path)
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "images");
        let result = self
            .inner
            .write_binary_if(request, data, context, capabilities)
            .await?;
        if image && result && self.pause_after_image.load(Ordering::SeqCst) {
            self.image_created.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(result)
    }

    /// 【表情测试】【删除故障】可在真实删除前失败或删除后暂停，以验证恢复边界
    /// @param request 删除请求；context 为目录；capabilities 为授权
    /// @returns 实际删除结果
    async fn remove_file(
        &self,
        request: FileRemovalRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<bool> {
        if self.fail_removal.load(Ordering::SeqCst) {
            bail!("fixture removal failure");
        }
        let result = self
            .inner
            .remove_file(request, context, capabilities)
            .await?;
        if self.pause_after_removal.load(Ordering::SeqCst) {
            self.removed.notify_one();
            std::future::pending::<()>().await;
        }
        Ok(result)
    }

    /// 【表情测试】【固定终端】尺寸规则不依赖测试进程是否有交互终端
    /// @returns 固定的 100 列和 40 行
    fn terminal_size(&self) -> Option<(u16, u16)> {
        Some((100, 40))
    }

    /// 【表情测试】【绘制捕获】记录真实 Lua 提交的路径，拒绝不存在的图片
    /// @param path 图片；size 为尺寸；context 为目录；capabilities 为授权
    /// @returns 绘制路径或可控错误
    async fn display_image(
        &self,
        path: String,
        size: Option<String>,
        _: SystemContext,
        capabilities: Capabilities,
    ) -> Result<DisplayedImage> {
        assert!(capabilities.binary.display_images);
        if self.fail_display.load(Ordering::SeqCst) {
            bail!("fixture display failure");
        }
        assert!(Path::new(&path).is_file(), "missing displayed file: {path}");
        self.displays.lock().unwrap().push((path.clone(), size));
        Ok(DisplayedImage { path })
    }
}

/// 【表情测试】【包构造】使用实际发布源码和兼容授权，仅追加显式测试时钟或入口
/// @param root 临时目录；extra 为业务设置；host 为文件宿主；suffix 为测试装配代码
/// @returns 原始包、有效设置和宿主组成的独立运行时
pub(super) fn runtime(
    root: &Path,
    extra: Value,
    host: Arc<dyn PluginHost>,
    suffix: &str,
) -> PluginRuntime {
    configured(root, extra, host, suffix, |_| {})
}

/// 【表情测试】【权限配置】允许测试独立撤销已声明的能力
/// @param root 根目录；extra 为设置；host 为宿主；suffix 为测试入口；grants 为权限修改
/// @returns 完整运行时
pub(super) fn configured(
    root: &Path,
    extra: Value,
    host: Arc<dyn PluginHost>,
    suffix: &str,
    grants: impl FnOnce(&mut Capabilities),
) -> PluginRuntime {
    let descriptor = descriptor(root, &AppConfig::default());
    let package = descriptor.runtime_package();
    let mut settings = descriptor.settings().clone();
    settings
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let manifest = package.manifest.clone();
    let mut allowed = descriptor.grants();
    grants(&mut allowed);
    let mut sources = package.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(suffix);
    PluginRuntime::load(
        PluginPackage::new(manifest, sources).unwrap(),
        settings,
        allowed,
        host,
    )
    .unwrap()
}

/// 【表情测试】【应用描述符】普通安装测试清单，显式绑定并授权隔离目录
/// @param root 临时目录；config 为真实应用配置
/// @returns 可以交给正式注册入口的描述符
pub(super) fn descriptor(
    root: &Path,
    config: &AppConfig,
) -> crate::plugins::discovery::PluginDescriptor {
    let paths = SaiPaths::for_tests(root);
    if !paths.config_dir.join("plugins/memes").exists() {
        super::example_support::install_custom("memes", &paths, |manifest| {
            let caps = &mut manifest.capabilities;
            caps.system.read_paths = ["builtin", "user", "recent", "input"]
                .into_iter()
                .map(|path| root.join(path).display().to_string())
                .collect();
            caps.binary.write_paths = ["user", "recent"]
                .into_iter()
                .map(|path| root.join(path).display().to_string())
                .collect();
            caps.system.remove_paths = [root.join("user").display().to_string()]
                .into_iter()
                .collect();
            caps.system.trash_paths = caps.system.remove_paths.clone();
        });
    }
    let settings = json!({"builtin_dirs":[root.join("builtin")],"user_dir":root.join("user"),"state_dir":root.join("recent")});
    crate::plugins::configure(config, &paths, "memes", settings).unwrap();
    crate::plugins::set_enabled(
        config,
        &paths,
        "memes",
        true,
        crate::plugins::GrantUpdate::Declared,
    )
    .unwrap();
    crate::plugins::discovery::find(config, &paths, "memes").unwrap()
}

/// 【表情测试】【可信上下文】固定本次会话和操作归属
/// @param root 临时工作目录
/// @returns 有写入权限的上下文
pub(super) fn context(root: &Path) -> InvocationContext {
    InvocationContext {
        workdir: root.display().to_string(),
        session_id: "memes-session".into(),
        storage_session_id: "memes-session".into(),
        operation_id: "memes-operation".into(),
        allow_writes: true,
        ..Default::default()
    }
}

/// 【表情测试】【工具执行】调用真实发布工具，并解析其公开 JSON 结果
/// @param runtime 实例；root 为目录；name 为工具；args 为参数
/// @returns 业务结果
pub(super) async fn call(runtime: &PluginRuntime, root: &Path, name: &str, args: Value) -> Value {
    let result = runtime.call_tool(name, args, context(root)).await.unwrap();
    serde_json::from_str(&result).unwrap()
}

/// 【表情测试】【条目样本】构造合法旧索引条目
/// @param id 完整标识；file 为相对路径；name 为名称
/// @returns 完整元数据
pub(super) fn item(id: &str, file: &str, name: &str) -> Value {
    json!({"id":id,"name":{"zh":name,"en":""},"file":file,"mime_type":"image/png","animated":false,
        "description":"企鹅 Linux","usage":"轻松聊天","avoid":"严肃求助","tags":["Linux"]})
}

/// 【表情测试】【索引装配】写入独立样本及其图片，不访问用户目录
/// @param root 测试根；source 为 builtin 或 user；items 为条目数组
/// @returns 无
pub(super) fn seed(root: &Path, source: &str, items: Vec<Value>) {
    let base = root.join(source).join("sai");
    std::fs::create_dir_all(base.join("images")).unwrap();
    for item in &items {
        std::fs::write(base.join(item["file"].as_str().unwrap()), b"fixture image").unwrap();
    }
    std::fs::write(
        base.join("index.json"),
        serde_json::to_vec(&json!({"library":"sai","version":2,"memes":items,"disabled_ids":[]}))
            .unwrap(),
    )
    .unwrap();
}

/// 【表情测试】【手工输入】使用不同字节生成可独立添加的图片与元数据
/// @param root 根目录；number 为区分内容的序号
/// @returns add_meme 输入
pub(super) fn addition(root: &Path, number: usize) -> Value {
    let path = root.join("input").join(format!("{number}.png"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("image {number}")).unwrap();
    json!({"image":path,"name_zh":format!("图片 {number}"),"description":"图像","usage":"聊天"})
}
