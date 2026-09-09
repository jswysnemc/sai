use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::{
    DirectoryEntry, DirectoryListing, FileInfo, FileReadRequest, FileText, HttpRequest,
    HttpResponse, PluginHost, ProcessOutput, ProcessRequest, SystemContext,
};
use sai_plugin_runtime::Capabilities;
#[cfg(unix)]
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub(super) struct DiagnosticHost {
    pub calls: Mutex<Vec<ProcessRequest>>,
    pub files: BTreeMap<String, String>,
    pub running: AtomicBool,
    pub timeouts: BTreeSet<String>,
    pub reads: Mutex<Vec<String>>,
}

impl Default for DiagnosticHost {
    /// 【诊断样本】【固定系统】创建无网络、无真实应用操作的 Linux 证据集。
    /// @returns 可观察的诊断宿主
    fn default() -> Self {
        let files=[
            ("/etc/os-release","PRETTY_NAME=\"Fixture Linux\"\n"),
            ("/proc/71/maps","01 0 0 /usr/lib/fcitx5/libwaylandim.so\n"),
            ("/proc/321/maps","01 0 0 /usr/lib/gtk-3.0/immodules/im-fcitx5.so\n"),
            ("/proc/321/cmdline","/usr/bin/example\0--ozone-platform=wayland\0"),
            ("/proc/321/environ","LANG=zh_CN.UTF-8\0WAYLAND_DISPLAY=wayland-0\0DISPLAY=:0\0GTK_IM_MODULE=fcitx\0XMODIFIERS=@im=fcitx\0SECRET=must-not-appear\0"),
            ("/proc/net/unix","000: 2 0 0 0001 01 902 /run/user/1000/wayland-0\n"),
            ("/usr/lib/gtk-3.0/3.0.0/immodules.cache","\"/usr/lib/im-fcitx5.so\" \"fcitx5\" \"Fcitx\" \"domain\" \"*\"\n"),
            ("/usr/lib/gtk-4.0/4.0.0/immodules.cache",""),
            ("/usr/share/applications/example.desktop","[Desktop Entry]\nExec=/usr/bin/example --ozone-platform=wayland\n"),
        ];
        Self {
            calls: Mutex::default(),
            files: files
                .into_iter()
                .map(|(path, text)| (path.into(), text.into()))
                .collect(),
            running: AtomicBool::new(true),
            timeouts: BTreeSet::new(),
            reads: Mutex::default(),
        }
    }
}

/// 【诊断样本】【目录条目】为模拟目录构造与正式宿主相同的条目结构。
/// @param path 完整路径；is_dir 为目录标记
/// @returns 可序列化条目
fn entry(path: &str, is_dir: bool) -> DirectoryEntry {
    DirectoryEntry {
        name: path.rsplit('/').next().unwrap().into(),
        path: path.into(),
        is_file: !is_dir,
        is_dir,
    }
}

#[async_trait]
impl PluginHost for DiagnosticHost {
    /// 【诊断样本】【环境查询】只返回授权的固定公开环境，不读取测试机环境。
    /// @param name 名称；capabilities 为有效授权
    /// @returns 样本环境值
    fn environment(&self, name: &str, capabilities: &Capabilities) -> Result<Option<String>> {
        capabilities.system.authorize_environment(name)?;
        Ok(match name {
            "HOME" => Some("/home/fixture"),
            "LANG" => Some("zh_CN.UTF-8"),
            "SHELL" => Some("/bin/sh"),
            "TERM" => Some("xterm"),
            "XDG_SESSION_TYPE" => Some("wayland"),
            _ => None,
        }
        .map(str::to_string))
    }

    /// 【诊断样本】【文件查询】从固定字典读取证据，缺少样本时明确失败。
    /// @param request 请求；context 为可信目录；capabilities 为授权
    /// @returns 固定文本
    async fn read_text(
        &self,
        request: FileReadRequest,
        _context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<FileText> {
        capabilities.system.check_read_request(&request.path)?;
        self.reads.lock().unwrap().push(request.path.clone());
        let text = self
            .files
            .get(&request.path)
            .ok_or_else(|| anyhow::anyhow!("missing fixture: {}", request.path))?;
        Ok(FileText {
            text: text.clone(),
            truncated: false,
        })
    }

    /// 【诊断样本】【目录查询】仅公开固定模块和桌面入口。
    /// @param path 目录；max_entries 为条数限制；context 为目录；capabilities 为授权
    /// @returns 固定列表
    async fn read_directory(
        &self,
        path: String,
        _max_entries: usize,
        _context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<DirectoryListing> {
        capabilities.system.check_read_request(&path)?;
        let entries = match path.as_str() {
            "/usr/lib" => vec![entry("/usr/lib/gtk-3.0", true)],
            "/usr/lib/gtk-3.0" => vec![entry("/usr/lib/gtk-3.0/immodules", true)],
            "/usr/lib/gtk-3.0/immodules" => {
                vec![entry("/usr/lib/gtk-3.0/immodules/im-fcitx5.so", false)]
            }
            "/usr/share/applications" => {
                vec![entry("/usr/share/applications/example.desktop", false)]
            }
            _ => vec![],
        };
        Ok(DirectoryListing {
            entries,
            truncated: false,
        })
    }

    /// 【诊断样本】【属性查询】包数据库锁样本固定存在。
    /// @param path 路径；context 为目录；capabilities 为授权
    /// @returns 文件属性或 None
    async fn file_info(
        &self,
        path: String,
        _context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<Option<FileInfo>> {
        capabilities.system.check_read_request(&path)?;
        Ok((path == "/var/lib/pacman/db.lck").then_some(FileInfo {
            is_file: true,
            is_dir: false,
            len: 0,
        }))
    }

    /// 【诊断样本】【模板执行】验证完整模板权限后返回样本，所有实际程序均禁止启动。
    /// @param request 模板请求；context 为可信权限；capabilities 为授权
    /// @returns 确定性进程输出
    async fn process(
        &self,
        request: ProcessRequest,
        context: SystemContext,
        capabilities: Capabilities,
    ) -> Result<ProcessOutput> {
        capabilities.system.process_command(
            &request.template,
            &request.parameters,
            context.allow_writes,
        )?;
        self.calls.lock().unwrap().push(request.clone());
        let timed_out = self.timeouts.contains(&request.template);
        let target = request.parameters["target"].as_str().unwrap_or("");
        let stdout = if timed_out {
            String::new()
        } else {
            match request.template.as_str() {
            "system-info"=>"Linux fixture kernel\n".into(),
            "command-path"=>format!("/usr/bin/{target}\n"),
            "processes"=>match target {
                "fcitx5"=>"71 /usr/bin/fcitx5\n".into(),
                "example" if self.running.load(Ordering::SeqCst)=>"321 /usr/bin/example --ozone-platform=wayland\n".into(),
                _=>String::new(),
            },
            "wayland-info"=>"zwp_text_input_manager_v3\n".into(),
            "fcitx-status"=>"2\n".into(),
            "locales"=>"C\nPOSIX\nzh_CN.utf8\nen_US.utf8\n".into(),
            "sockets"=>"u_str ESTAB 0 0 * 902 * 910 users:((example,pid=321,fd=3))\n".into(),
            "package-owner"=>"/usr/bin/example is owned by fixture-app 1.0\n".into(),
            "package-files"=>"fixture-app /usr/lib/electron/app.asar\nfixture-app /usr/bin/example\n".into(),
            "systemd-user"=>"active\n".into(),
            "audio-status"=>"Audio devices\n".into(),
            "pci"=>"00:01 VGA compatible controller\n Kernel driver in use: fixture\n".into(),
            "network-addresses"=>"eth0 UP 192.168.50.1/24\n".into(),
            "resolver-status"=>"DNS current server\n".into(),
            "disk-usage"=>"Filesystem Type Size Used Avail\nfixture ext4 1T 1G 999G\n".into(),
            "macos-info"=>"ProductName: macOS\nProductVersion: 15.0\n".into(),
            "recent-logs"=>"example input error\npipewire audio failed\npacman warning\nportal failed\nunrelated success\n".into(),
            "app-version"=>"example 1.2.3\n".into(),
            _=>bail!("unexpected fixture template: {}",request.template),
        }
        };
        Ok(ProcessOutput {
            status: if timed_out { None } else { Some(0) },
            stdout,
            stderr: String::new(),
            timed_out,
            stdout_truncated: false,
            stderr_truncated: false,
        })
    }

    /// 【诊断样本】【禁止网络】诊断证据测试只允许本地系统替身。
    /// @param request 请求；capabilities 为授权；allow_writes 为权限
    /// @returns 明确错误
    async fn http(
        &self,
        _request: HttpRequest,
        _capabilities: Capabilities,
        _allow_writes: bool,
    ) -> Result<HttpResponse> {
        bail!("diagnostic fixture has no HTTP access")
    }
}

/// 【诊断样本】【结果解析】统一解析工具返回 JSON。
/// @param output 工具输出
/// @returns JSON 结果
#[cfg(unix)]
pub(super) fn parsed(output: String) -> Value {
    serde_json::from_str(&output).unwrap()
}
