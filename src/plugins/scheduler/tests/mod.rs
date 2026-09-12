mod grants;
mod language;
mod lifecycle;
mod operations;
mod quotas;
mod storage;

use super::{
    process::{WorkerHandle, WorkerLauncher},
    record::JobRecord,
};
use crate::{
    config::AppConfig,
    paths::SaiPaths,
    plugins::discovery::{discover, PluginDescriptor},
};
use anyhow::Result;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

const SOURCE: &str = r#"
    --- 【调度样本】【记录调用】保存执行计数和传入文本
    --- @param args string 命令文本
    --- @return string 原始文本
    local function deliver(args)
        local count=sai.storage.plugin.get("calls")
        if count==nil or count==sai.json.null then count=0 end
        sai.storage.plugin.set("calls",count+1)
        sai.storage.plugin.set("payload",args)
        return args
    end
    sai.register_command({name="deliver",description="deliver",access="writes",execute=deliver})
    sai.register_command({name="hold",description="hold",access="writes",execute=function(args)
        deliver(args)
        while true do sai.scheduler.list() end
    end})
    sai.register_command({name="fail",description="fail",access="writes",execute=function() error("scheduled fixture failure") end})
    sai.register_command({name="large",description="large",execute=function() return string.rep("汉",10000) end})
"#;

/// 【调度样本】【隔离环境】创建显式授权的本地包，避免访问真实应用配置。
/// @returns 临时目录、宿主路径及发现描述符
fn fixture() -> (tempfile::TempDir, SaiPaths, PluginDescriptor) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let package = paths.config_dir.join("plugins/schedule-fixture");
    std::fs::create_dir_all(&package).unwrap();
    let capabilities = json!({"system":{"schedule":true,"plugin_storage":true}});
    std::fs::write(
        package.join("sai-plugin.json"),
        serde_json::to_vec(&json!({
            "api_version":1,"id":"schedule-fixture","version":"1.0.0","name":"Schedule fixture",
            "description":"Scheduler validation","entry":"init.lua","capabilities":capabilities,
            "limits":{"instructions":20000000,"timeout_ms":2000,"system_calls":4096}
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(package.join("init.lua"), SOURCE).unwrap();
    std::fs::write(
        paths.config_dir.join("plugins.jsonc"),
        serde_json::to_vec(&json!({
            "plugins":{"schedule-fixture":{"enabled":true,"grants":capabilities}}
        }))
        .unwrap(),
    )
    .unwrap();
    let descriptor = discover(&AppConfig::default(), &paths)
        .plugins
        .into_iter()
        .find(|descriptor| descriptor.package.manifest.id == "schedule-fixture")
        .unwrap();
    (root, paths, descriptor)
}

#[derive(Default)]
struct Launcher {
    spawned: AtomicUsize,
    fail: bool,
}
struct Handle;

impl WorkerHandle for Handle {
    /// 【调度样本】【进程标识】以当前进程模拟工作入口的启动握手。
    /// @returns 当前测试进程 PID
    fn id(&self) -> u32 {
        std::process::id()
    }
    /// 【调度样本】【发布确认】样本不启动或终止实际子进程。
    /// @returns 无
    fn commit(self: Box<Self>) {}
}

impl WorkerLauncher for Launcher {
    /// 【调度样本】【启动捕获】计数并模拟启动失败，工作过程由测试显式调用。
    /// @param record 待启动的可信记录
    /// @returns 无副作用的进程守卫
    fn spawn(&self, _record: &JobRecord) -> Result<Box<dyn WorkerHandle>> {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            anyhow::bail!("fixture spawn failed");
        }
        Ok(Box::new(Handle))
    }
}

/// 【调度样本】【任务创建】使用正式预检和发布流程，仅替换进程创建边界。
/// @param paths 应用路径；descriptor 为描述；command 为命令；due_at 为时间；launcher 为测试启动器
/// @returns 持久任务
fn create(
    paths: &SaiPaths,
    descriptor: &PluginDescriptor,
    command: &str,
    due_at: i64,
    launcher: &Launcher,
) -> sai_plugin_runtime::host::ScheduledTask {
    super::schedule(
        paths,
        &descriptor.package.manifest.id,
        &descriptor.revision().unwrap(),
        sai_plugin_runtime::host::ScheduleRequest {
            due_at,
            command: command.into(),
            arguments: "双语 payload".into(),
        },
        &sai_plugin_runtime::host::SystemContext {
            workdir: paths.config_dir.display().to_string(),
            allow_writes: true,
        },
        launcher,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap()
}
