use super::discovery::{diagnostic, discover, PluginDiagnostic};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use sai_plugin_runtime::{TuiStatusContext, TuiStatusLine, TuiStatusRuntime};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

const MAX_RENDERERS: usize = 8;

/// 【底栏插件】【结果缓存】只保存最近一次状态与结果，错误由终端按次领取。
#[derive(Debug, Default)]
struct Cache {
    completed: Option<(TuiStatusContext, Option<TuiStatusLine>)>,
    diagnostics: Vec<PluginDiagnostic>,
}

/// 【底栏插件】【异步入口】绘制线程只提交快照、读取缓存，不执行 Lua 或文件读取。
#[derive(Clone, Debug)]
pub(crate) struct TuiStatusRenderer {
    pending: watch::Sender<Option<TuiStatusContext>>,
    cache: Arc<Mutex<Cache>>,
}

impl TuiStatusRenderer {
    /// 【底栏插件】【启动】创建独立工作线程，源码与设置在本次输入循环中保持一致。
    /// @param config 主配置；paths 为插件安装与配置目录
    /// @returns 可克隆的缓存句柄；全部句柄释放后工作线程自动退出
    pub(crate) fn start(config: AppConfig, paths: SaiPaths) -> Self {
        let (pending, receiver) = watch::channel(None);
        let cache = Arc::new(Mutex::new(Cache::default()));
        let worker_cache = cache.clone();
        let result = std::thread::Builder::new()
            .name("sai-tui-status".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(run(config, paths, receiver, worker_cache)),
                    Err(error) => report(&worker_cache, "tui_status", error.into()),
                }
            });
        if let Err(error) = result {
            report(&cache, "tui_status", error.into());
        }
        Self { pending, cache }
    }

    /// 【底栏插件】【快照提交】合并重复输入，后台只处理最新快照，绘制从不等待计算。
    /// @param context 当前终端与模型状态
    /// @returns 与当前快照一致的结果；尚未完成或失败时回退原生底栏
    pub(crate) fn render(&self, context: TuiStatusContext) -> Option<TuiStatusLine> {
        self.pending.send_if_modified(|pending| {
            if pending.as_ref() == Some(&context) {
                return false;
            }
            *pending = Some(context.clone());
            true
        });
        self.cache
            .lock()
            .ok()?
            .completed
            .as_ref()
            .filter(|(input, _)| input == &context)
            .and_then(|(_, line)| line.clone())
    }

    /// 【底栏插件】【刷新等待】告知输入循环是否需要继续轮询绘制缓存。
    /// @returns 有活动工作线程时为 true，无插件或全部失败后为 false
    pub(crate) fn active(&self) -> bool {
        !self.pending.is_closed()
    }

    /// 【底栏插件】【诊断领取】一次性交付错误，不从后台线程直接写入终端。
    /// @returns 尚未展示的诊断列表
    pub(crate) fn take_diagnostics(&self) -> Vec<PluginDiagnostic> {
        self.cache
            .lock()
            .map(|mut cache| std::mem::take(&mut cache.diagnostics))
            .unwrap_or_default()
    }
}

/// 【底栏插件】【错误缓存】记录独立插件错误，保证其他插件和输入循环继续工作。
/// @param cache 展示缓存；id 为插件标识；error 为错误链；返回无
fn report(cache: &Mutex<Cache>, id: &str, error: anyhow::Error) {
    if let Ok(mut cache) = cache.lock() {
        cache.diagnostics.push(diagnostic(id, error));
    }
}

/// 【底栏插件】【后台调度】稳定排序选取第一个有效布局，出错插件在本轮停用。
/// @param config 主配置；paths 为安装路径；requests 为合并快照队列；cache 为显示结果
/// @returns 无；没有可用插件或终端句柄全部释放时结束
async fn run(
    config: AppConfig,
    paths: SaiPaths,
    mut requests: watch::Receiver<Option<TuiStatusContext>>,
    cache: Arc<Mutex<Cache>>,
) {
    // 1. 【底栏插件】【受限加载】只读取启用且明确授权底栏能力的插件
    let found = discover(&config, &paths);
    let mut renderers = Vec::new();
    for descriptor in found
        .plugins
        .into_iter()
        .filter(|item| {
            item.setting.enabled && item.capabilities().tui_status && item.grants().tui_status
        })
        .take(MAX_RENDERERS)
    {
        let id = descriptor.package.manifest.id.clone();
        match TuiStatusRuntime::load(
            descriptor.runtime_package(),
            descriptor.settings().clone(),
            descriptor.grants(),
        ) {
            Ok(runtime) => renderers.push((id, runtime)),
            Err(error) => report(&cache, &id, error),
        }
    }
    // 2. 【底栏插件】【合并刷新】只在状态变化时运行回调，不积累逐键绘制任务
    while !renderers.is_empty() && requests.changed().await.is_ok() {
        let Some(context) = requests.borrow_and_update().clone() else {
            continue;
        };
        let mut output = None;
        let mut index = 0;
        while index < renderers.len() {
            let (id, runtime) = &renderers[index];
            match runtime.render(&context).await {
                Ok(Some(line)) => {
                    output = Some(line);
                    break;
                }
                Ok(None) => index += 1,
                Err(error) => {
                    report(&cache, id, error);
                    renderers.remove(index);
                }
            }
        }
        if let Ok(mut cache) = cache.lock() {
            cache.completed = Some((context, output));
        }
    }
}
