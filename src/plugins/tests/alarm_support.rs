use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::*;
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub(super) const NOW: i64 = 1_700_000_000;

#[derive(Default)]
pub(super) struct AlarmHost {
    pub tasks: Mutex<Vec<ScheduledTask>>,
    pub notices: Mutex<Vec<NotificationRequest>>,
    pub paths: Mutex<Vec<String>>,
    pub contexts: Mutex<Vec<SystemContext>>,
    pub fail_notice: AtomicBool,
    pub wait_notice: AtomicBool,
    pub active_notice: Arc<AtomicBool>,
    pub notice_released: Arc<tokio::sync::Notify>,
}

struct ActiveNotice(Arc<AtomicBool>, Arc<tokio::sync::Notify>);

impl Drop for ActiveNotice {
    /// 【闹钟测试】【投递释放】记录超时后通知 Future 已释放。
    /// @returns 无
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
        self.1.notify_one();
    }
}

#[async_trait]
impl PluginHost for AlarmHost {
    /// 【闹钟测试】【网络隔离】闹钟业务不应发起 HTTP 请求。
    /// @param request 请求；capabilities 为授权；allow_writes 为写入权限
    /// @returns 固定拒绝
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        bail!("alarm fixture must not access the network")
    }

    /// 【闹钟测试】【调度宿主】记录真实 Lua 请求，模拟持久任务状态。
    /// @param request 操作；context 为可信目录；capabilities 为权限
    /// @returns 对应任务结果
    async fn scheduler(
        &self,
        request: SchedulerRequest,
        context: &SystemContext,
        capabilities: &Capabilities,
    ) -> Result<SchedulerResponse> {
        request.authorize(capabilities, context.allow_writes)?;
        self.contexts.lock().unwrap().push(context.clone());
        let mut tasks = self.tasks.lock().unwrap();
        Ok(match request {
            SchedulerRequest::Schedule(request) => {
                let next = task(tasks.len(), ScheduledStatus::Scheduled, &request.arguments);
                let next = ScheduledTask {
                    command: request.command,
                    due_at: request.due_at,
                    ..next
                };
                tasks.push(next.clone());
                SchedulerResponse::Task(Some(next))
            }
            SchedulerRequest::List(options) => SchedulerResponse::Tasks(
                tasks
                    .iter()
                    .skip(options.offset)
                    .take(options.limit)
                    .cloned()
                    .collect(),
            ),
            SchedulerRequest::Get(id) => {
                SchedulerResponse::Task(tasks.iter().find(|task| task.id == id).cloned())
            }
            SchedulerRequest::Cancel(id) => {
                let found = tasks
                    .iter_mut()
                    .find(|task| task.id == id && task.status.is_active());
                let changed = found.is_some();
                if let Some(task) = found {
                    task.status = ScheduledStatus::Cancelled;
                    task.finished_at = Some(NOW);
                }
                SchedulerResponse::Changed(changed)
            }
            SchedulerRequest::Resume(_) => bail!("unexpected resume in alarm business"),
        })
    }

    /// 【闹钟测试】【通知宿主】保存通道请求并模拟失败、等待及释放。
    /// @param request 通知；context 为可信权限；capabilities 为能力
    /// @returns 完成通道
    async fn notify(
        &self,
        request: NotificationRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<NotificationDelivery> {
        assert!(context.allow_writes && capabilities.system.notify);
        let delivery = NotificationDelivery {
            desktop: request.desktop,
            sound: request.sound.is_some(),
        };
        self.notices.lock().unwrap().push(request);
        self.active_notice.store(true, Ordering::SeqCst);
        let _guard = ActiveNotice(self.active_notice.clone(), self.notice_released.clone());
        if self.fail_notice.load(Ordering::SeqCst) {
            bail!("fixture audio failed");
        }
        if self.wait_notice.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        Ok(delivery)
    }

    /// 【闹钟测试】【路径解析】只允许明确准备的样本音频。
    /// @param path 路径；context 为可信目录；capabilities 为读取授权
    /// @returns 平台绝对路径
    async fn real_path(
        &self,
        path: String,
        _context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<String> {
        capabilities.system.check_read_request(&path)?;
        self.paths.lock().unwrap().push(path.clone());
        if path == "missing.wav" {
            bail!("fixture file not found");
        }
        Ok(audio_path(&path))
    }

    /// 【闹钟测试】【文件属性】返回普通文件、空文件或过大文件样本。
    /// @param path 路径；context 为目录；capabilities 为授权
    /// @returns 文件属性
    async fn file_info(
        &self,
        path: String,
        _context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<Option<FileInfo>> {
        capabilities.system.check_read_request(&path)?;
        Ok(Some(FileInfo {
            is_file: !path.ends_with("folder.wav"),
            is_dir: path.ends_with("folder.wav"),
            len: if path.ends_with("large.wav") {
                8 * 1024 * 1024 + 1
            } else if path.ends_with("empty.wav") {
                0
            } else {
                128
            },
        }))
    }
}

/// 【闹钟测试】【绝对路径】构造样本路径，兼容 Windows 前缀。
/// @param name 音频名称
/// @returns 绝对路径
pub(super) fn audio_path(name: &str) -> String {
    if cfg!(windows) {
        format!("C:/fixture/{name}")
    } else {
        format!("/fixture/{name}")
    }
}

/// 【闹钟测试】【记录样本】生成可经过宿主结果校验的独立任务。
/// @param index 序号；status 为状态；arguments 为命令输入
/// @returns 任务记录
pub(super) fn task(index: usize, status: ScheduledStatus, arguments: &str) -> ScheduledTask {
    ScheduledTask {
        id: format!("job-{index:032x}"),
        command: "deliver".into(),
        arguments: arguments.into(),
        due_at: NOW + 30,
        status,
        pid: Some(123),
        created_at: NOW,
        finished_at: if status.is_active() { None } else { Some(NOW) },
        output: None,
        output_truncated: false,
        error: None,
    }
}

/// 【闹钟测试】【固定时间】基于通用 ISO 接口固定时钟格式，避免依赖测试机器时区。
pub(super) const FIXED_TIME: &str = r#"
    sai.time.now=function() return 1700000000 end
    sai.time.local_format=function(format, seconds)
        local text=sai.time.iso(seconds or sai.time.now(), 0)
        if format=="%H %M %S" then return text:sub(12,19):gsub(":"," ") end
        assert(format=="%Y-%m-%d %H:%M:%S")
        return text:sub(1,19):gsub("T"," ")
    end
"#;

/// 【闹钟测试】【实际源码】取得真实发布包并追加测试时钟。
/// @returns 带固定时钟的源码快照
pub(super) fn package() -> PluginPackage {
    let package = crate::plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == "alarm")
        .expect("alarm plugin is missing");
    let mut sources = package.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(FIXED_TIME);
    PluginPackage::new(package.manifest, sources).unwrap()
}

/// 【闹钟测试】【业务实例】使用实际包和给定权限创建隔离运行时。
/// @param host 宿主；grants 为可选的显式授权
/// @returns 运行时
pub(super) fn runtime(host: Arc<AlarmHost>, grants: Option<Capabilities>) -> PluginRuntime {
    let package = package();
    let grants = grants.unwrap_or_else(|| package.manifest.capabilities.clone());
    PluginRuntime::load(package, json!({}), grants, host).unwrap()
}

/// 【闹钟测试】【工具执行】在独立可信目录中执行实际 Lua 工具。
/// @param plugin 运行时；name 为工具；args 为参数；writes 为可信写入权限
/// @returns JSON 结果或错误
pub(super) async fn call(
    plugin: &PluginRuntime,
    name: &str,
    args: Value,
    writes: bool,
) -> Result<Value> {
    let output = plugin
        .call_tool(
            name,
            args,
            InvocationContext {
                workdir: audio_path("work"),
                allow_writes: writes,
                ..Default::default()
            },
        )
        .await?;
    Ok(serde_json::from_str(&output)?)
}
