#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{
    env,
    process::{Command, ExitCode},
};

/// 【桌面浏览器】【启动桥】把 Sai 的浏览器参数转交给独立 Electron 进程
/// 参数：读取命令行及桌面端设置的运行环境；返回子进程退出码
fn main() -> ExitCode {
    match launch() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("【桌面浏览器】【启动失败】{error}");
            ExitCode::FAILURE
        }
    }
}

/// 【桌面浏览器】【进程创建】兼容开发入口和安装包，保留 Sai 管理的用户目录
/// 参数：无显式参数，使用环境与命令行；返回 Electron 退出码或错误
fn launch() -> Result<u8, Box<dyn std::error::Error>> {
    let executable = env::var_os("SAI_DESKTOP_EXECUTABLE").ok_or("missing desktop executable")?;
    let mut command = Command::new(executable);
    // 1. 【桌面浏览器】【入口选择】开发模式需要显式传入工程入口，安装包自行定位入口
    if let Some(entry) = env::var_os("SAI_DESKTOP_DEV_ENTRY") {
        command.arg(entry);
    }
    command.arg("--sai-browser-host");
    for arg in env::args_os().skip(1) {
        // 2. 【桌面浏览器】【离屏渲染】Electron 自行管理离屏窗口，不启用 Chrome 独立无头入口
        if !arg.to_string_lossy().starts_with("--headless") {
            command.arg(arg);
        }
    }
    command.env("SAI_DESKTOP_LAUNCHER_PID", std::process::id().to_string());
    command.env_remove("ELECTRON_RUN_AS_NODE");
    let status = command.spawn()?.wait()?;
    Ok(status.code().unwrap_or(1).clamp(0, 255) as u8)
}
