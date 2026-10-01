//! 查找本机 Chromium 系浏览器并以远程调试模式启动。

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};

/// 指定浏览器可执行文件的环境变量，优先级最高。
pub(super) const EXECUTABLE_ENV: &str = "SAI_BROWSER_EXECUTABLE";
/// 设为 `1` 时以有界面模式启动，便于本机调试。
pub(super) const HEADED_ENV: &str = "SAI_BROWSER_HEADED";
/// 默认视口宽度（CSS 像素）。
pub(super) const DEFAULT_WIDTH: u32 = 1280;
/// 默认视口高度（CSS 像素）。
pub(super) const DEFAULT_HEIGHT: u32 = 800;
/// 等待调试端口就绪的上限。
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

/// PATH 中按顺序尝试的浏览器命令名。
const PATH_CANDIDATES: &[&str] = &[
    "google-chrome-stable",
    "google-chrome",
    "chromium",
    "chromium-browser",
    "microsoft-edge-stable",
    "microsoft-edge",
    "brave-browser",
    "brave",
    "chrome",
    "msedge",
];

/// 已启动的浏览器进程与调试入口。
pub(super) struct LaunchedBrowser {
    /// 浏览器主进程，释放时强制结束
    pub(super) child: Child,
    /// 浏览器级 WebSocket 调试地址
    pub(super) websocket_url: String,
    /// 本次启动专用的临时用户目录，随结构体释放而删除
    pub(super) _profile: tempfile::TempDir,
}

/// 【内置浏览器】【可执行文件定位】按环境变量、PATH 与各平台默认安装位置查找浏览器。
/// @returns 浏览器可执行文件路径
pub(super) fn find_executable() -> Result<PathBuf> {
    // 1. 环境变量显式指定时只认这一个路径
    if let Some(value) = std::env::var_os(EXECUTABLE_ENV).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
        bail!(
            "{EXECUTABLE_ENV} points to a missing file: {}",
            path.display()
        );
    }
    // 2. 在 PATH 中按常见命令名查找
    if let Some(path) = PATH_CANDIDATES.iter().find_map(|name| search_path(name)) {
        return Ok(path);
    }
    // 3. 回退到 macOS 与 Windows 的默认安装位置
    if let Some(path) = platform_install_paths()
        .into_iter()
        .find(|path| path.is_file())
    {
        return Ok(path);
    }
    bail!("No Chromium-based browser found. Install Chrome/Chromium/Edge or set {EXECUTABLE_ENV}.")
}

/// 【内置浏览器】【PATH 查找】在 PATH 各目录中查找指定命令。
/// @param name 为命令名
/// @returns 找到的可执行文件路径
fn search_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }
        let exe = dir.join(format!("{name}.exe"));
        (cfg!(windows) && exe.is_file()).then_some(exe)
    })
}

/// 【内置浏览器】【默认安装位置】列出当前平台常见的浏览器安装路径。
/// @returns 候选路径列表
fn platform_install_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if cfg!(target_os = "macos") {
        for app in [
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Chromium.app/Contents/MacOS/Chromium",
            "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "Brave Browser.app/Contents/MacOS/Brave Browser",
        ] {
            paths.push(Path::new("/Applications").join(app));
        }
    }
    if cfg!(windows) {
        for var in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
            let Some(root) = std::env::var_os(var) else {
                continue;
            };
            let root = PathBuf::from(root);
            paths.push(root.join("Google/Chrome/Application/chrome.exe"));
            paths.push(root.join("Microsoft/Edge/Application/msedge.exe"));
            paths.push(root.join("Chromium/Application/chrome.exe"));
        }
    }
    paths
}

/// 【内置浏览器】【启动参数】组装远程调试模式下的浏览器命令行参数。
/// @param profile 为临时用户目录；headed 为是否显示窗口
/// @returns 参数列表
pub(super) fn launch_args(profile: &Path, headed: bool) -> Vec<String> {
    let mut args = vec![
        // 端口填 0 由浏览器自选空闲端口，并写入用户目录下的 DevToolsActivePort
        "--remote-debugging-port=0".to_string(),
        "--remote-debugging-address=127.0.0.1".to_string(),
        format!("--user-data-dir={}", profile.display()),
        format!("--window-size={DEFAULT_WIDTH},{DEFAULT_HEIGHT}"),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-component-update".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-extensions".to_string(),
        "--disable-sync".to_string(),
        "--disable-dev-shm-usage".to_string(),
        "--disable-features=Translate,MediaRouter".to_string(),
        "--password-store=basic".to_string(),
        "--use-mock-keychain".to_string(),
        "--mute-audio".to_string(),
    ];
    if !headed {
        args.push("--headless=new".to_string());
        args.push("--hide-scrollbars".to_string());
    }
    // root 用户下 Chromium 拒绝启用沙箱，只能显式关闭
    if running_as_root() {
        args.push("--no-sandbox".to_string());
    }
    args.push("about:blank".to_string());
    args
}

/// 【内置浏览器】【权限判断】判断当前进程是否以 root 身份运行。
/// @returns root 时为 true；非 Unix 平台恒为 false
fn running_as_root() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid 不读写内存，始终成功
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// 【内置浏览器】【进程启动】启动浏览器并等待调试入口写出。
/// @returns 浏览器进程、调试地址与临时用户目录
pub(super) async fn launch() -> Result<LaunchedBrowser> {
    // 1. 定位可执行文件并准备隔离的临时用户目录
    let executable = find_executable()?;
    let profile = tempfile::Builder::new()
        .prefix("sai-browser-")
        .tempdir()
        .context("create browser profile directory")?;
    let headed = std::env::var(HEADED_ENV).is_ok_and(|value| value.trim() == "1");
    // 2. 启动进程；标准输出与错误输出丢弃，避免管道写满阻塞浏览器
    let mut command = Command::new(&executable);
    command
        .args(launch_args(profile.path(), headed))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    bind_to_parent_lifetime(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("launch browser {}", executable.display()))?;
    // 3. 轮询 DevToolsActivePort，浏览器提前退出时立即报错
    let websocket_url = wait_for_endpoint(&mut child, profile.path()).await?;
    Ok(LaunchedBrowser {
        child,
        websocket_url,
        _profile: profile,
    })
}

/// 【内置浏览器】【生命周期绑定】Linux 下父进程退出时让浏览器收到 SIGTERM。
/// @param command 为待启动命令
/// @returns 无
fn bind_to_parent_lifetime(command: &mut Command) {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: pre_exec 闭包只调用异步信号安全的 prctl
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = command;
    }
}

/// 【内置浏览器】【调试入口等待】读取用户目录中的端口文件并拼出 WebSocket 地址。
/// @param child 为浏览器进程；profile 为用户目录
/// @returns 浏览器级调试地址
async fn wait_for_endpoint(child: &mut Child, profile: &Path) -> Result<String> {
    let file = profile.join("DevToolsActivePort");
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Ok(content) = tokio::fs::read_to_string(&file).await {
            if let Some(url) = parse_active_port(&content) {
                return Ok(url);
            }
        }
        if let Some(status) = child.try_wait()? {
            bail!("browser exited before DevTools became ready ({status})");
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = child.kill().await;
            bail!("browser DevTools endpoint not ready within {STARTUP_TIMEOUT:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// 【内置浏览器】【端口文件解析】解析 DevToolsActivePort 的端口与路径两行。
/// @param content 为文件内容
/// @returns 完整 WebSocket 地址；内容不完整时为空
pub(super) fn parse_active_port(content: &str) -> Option<String> {
    let mut lines = content.lines();
    let port = lines.next()?.trim().parse::<u16>().ok()?;
    let path = lines.next()?.trim();
    if port == 0 || !path.starts_with('/') {
        return None;
    }
    Some(format!("ws://127.0.0.1:{port}{path}"))
}
