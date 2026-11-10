//! Windows 沙箱实际源码的独立交叉编译与原生验证入口。
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod sandbox;

#[cfg(windows)]
mod native;

/// 【Windows沙箱】【回归入口】以内部启动参数运行真实隔离模块；无参数，返回执行结果。
fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    if std::env::args().nth(1).as_deref() == Some(sandbox::windows::WORKER_ARG) {
        let payload = std::env::args()
            .nth(2)
            .ok_or_else(|| anyhow::anyhow!("missing payload"))?;
        std::process::exit(sandbox::windows::serve(&payload)? as i32);
    }
    #[cfg(windows)]
    native::run()?;
    Ok(())
}
