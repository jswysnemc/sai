use super::{ToolRegistry, ToolSpec};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use tokio::process::Command;

const MAX_COMMAND_OUTPUT_CHARS: usize = 20_000;
const SEARCH_TIMEOUT_SECONDS: u64 = 30;

pub fn register(registry: &mut ToolRegistry, config: &AppConfig, paths: &SaiPaths) {
    register_readonly(registry, config, paths);
    super::edit_file::register(registry);
}

pub fn register_readonly(registry: &mut ToolRegistry, config: &AppConfig, paths: &SaiPaths) {
    registry.register(ToolSpec::new(
        "check_os_info",
        t("Check basic read-only OS, shell, desktop session, kernel, host, and package-manager context. For concrete Linux input method issues, prefer linux_input_method_diagnose.", "查看只读基础系统信息，包括 OS、shell、桌面会话、内核、主机和包管理器上下文。排查具体 Linux 输入法问题时优先使用 linux_input_method_diagnose。"),
        json!({"type":"object","properties":{},"additionalProperties":false}),
        |_| async move { check_os_info() },
    ));
    super::file_read::register(registry, config.clone(), paths.clone());
    registry.register(ToolSpec::new(
        "glob",
        t("Find files by case-insensitive glob pattern under a directory. Defaults to workspace; use ~ or /home for user files, or / for protected global search.", "在目录下按大小写不敏感 glob 模式查找文件。默认工作区；查用户文件用 ~ 或 /home，受保护的全局搜索可用 /。"),
        json!({"type":"object","properties":{"path":{"type":"string","description": t("Directory to search. Defaults to workspace; use ~ or /home for user files, or / for protected global search.", "搜索目录，默认工作区；查用户文件用 ~ 或 /home，受保护的全局搜索可用 /。")},"pattern":{"type":"string","description": t("Case-insensitive glob pattern, for example *ai*test*.", "大小写不敏感 Glob 模式，例如 *ai*测试*。")},"max_results":{"type":"integer","description": t("Maximum results.", "最多结果数。")}},"required":["pattern"],"additionalProperties":false}),
        |args| async move { glob_files(args).await },
    ));
    registry.register(ToolSpec::new(
        "grep",
        t("Search file contents using ripgrep under a directory or single file. Defaults to workspace; use ~ or /home for user files, or / for protected global search. No matches are returned as an empty ok result.", "在目录或单个文件中用 ripgrep 搜索内容。默认工作区；查用户文件用 ~ 或 /home，受保护的全局搜索可用 /。无匹配会作为成功的空结果返回。"),
        json!({"type":"object","properties":{"path":{"type":"string","description": t("Directory or file to search. Defaults to workspace; use ~ or /home for user files, or / for protected global search.", "要搜索的目录或文件，默认工作区；查用户文件用 ~ 或 /home，受保护的全局搜索可用 /。")},"pattern":{"type":"string","description": t("Regex pattern.", "正则模式。")},"include":{"type":"string","description": t("Optional case-insensitive file glob filter.", "可选大小写不敏感文件 glob 过滤。")},"max_results":{"type":"integer","description": t("Maximum matches.", "最多匹配数。")}},"required":["pattern"],"additionalProperties":false}),
        |args| async move { grep_text(args).await },
    ));
}

fn check_os_info() -> Result<String> {
    let mut env = BTreeMap::new();
    for key in [
        "SHELL",
        "TERM",
        "LANG",
        "PATH",
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_TYPE",
        "DESKTOP_SESSION",
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "COMSPEC",
        "USERPROFILE",
        "USERNAME",
        "COMPUTERNAME",
        "OS",
        "PROCESSOR_ARCHITECTURE",
        "WT_SESSION",
    ] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                env.insert(key, value);
            }
        }
    }
    let os_release = read_small_file("/etc/os-release");
    let arch_release = read_small_file("/etc/arch-release").is_some();
    let debian_version = read_small_file("/etc/debian_version");
    let fedora_release = read_small_file("/etc/fedora-release");
    let proc_version = read_small_file("/proc/version");
    let proc_cmdline = read_small_file("/proc/cmdline");
    let macos_system_version = read_small_file("/System/Library/CoreServices/SystemVersion.plist");
    let macos = parse_macos_system_version(macos_system_version.as_deref());
    let package_manager_guess = package_manager_guess(
        &os_release,
        arch_release,
        debian_version.is_some(),
        fedora_release.is_some(),
        macos_system_version.is_some(),
    );
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "platform": std::env::consts::OS,
        "os_release": os_release,
        "arch_release": arch_release,
        "debian_version": debian_version,
        "fedora_release": fedora_release,
        "macos": macos,
        "kernel_version": proc_version,
        "kernel_cmdline": proc_cmdline,
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
        "family": std::env::consts::FAMILY,
        "username": std::env::var("USER").ok().or_else(|| std::env::var("USERNAME").ok()),
        "hostname": read_small_file("/etc/hostname")
            .map(|value| value.trim().to_string())
            .or_else(|| std::env::var("COMPUTERNAME").ok()),
        "env": env,
        "package_manager_guess": package_manager_guess,
        "notes": [
            "This tool is read-only and does not execute shell commands.",
            "This only reports basic OS context. For concrete Linux input method issues, use linux_input_method_diagnose."
        ],
    }))?)
}

fn read_small_file(path: &str) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn package_manager_guess(
    os_release: &Option<String>,
    arch_release: bool,
    debian_version: bool,
    fedora_release: bool,
    macos: bool,
) -> Vec<&'static str> {
    let lower = os_release
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut managers = Vec::new();
    if arch_release || lower.contains("id=arch") || lower.contains("id_like=arch") {
        managers.push("pacman");
    }
    if debian_version
        || lower.contains("id=debian")
        || lower.contains("id=ubuntu")
        || lower.contains("id_like=debian")
    {
        managers.push("apt");
    }
    if fedora_release || lower.contains("id=fedora") || lower.contains("id_like=fedora") {
        managers.push("dnf");
    }
    if macos || std::env::consts::OS == "macos" {
        if Path::new("/opt/homebrew").exists() || Path::new("/usr/local/Homebrew").exists() {
            managers.push("brew");
        }
        if Path::new("/opt/local").exists() {
            managers.push("port");
        }
        if !managers
            .iter()
            .any(|manager| matches!(*manager, "brew" | "port"))
        {
            managers.push("brew");
        }
    }
    if managers.is_empty() {
        managers.push("unknown");
    }
    managers
}

fn parse_macos_system_version(raw: Option<&str>) -> Value {
    let Some(raw) = raw else {
        return Value::Null;
    };
    json!({
        "product_name": plist_value(raw, "ProductName"),
        "product_version": plist_value(raw, "ProductVersion"),
        "product_build_version": plist_value(raw, "ProductBuildVersion"),
    })
}

fn plist_value(raw: &str, key: &str) -> Option<String> {
    let marker = format!("<key>{key}</key>");
    let after_key = raw.split(&marker).nth(1)?;
    let after_string = after_key.split("<string>").nth(1)?;
    after_string
        .split("</string>")
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// 按 Glob 模式查找文件。
///
/// 参数:
/// - `args`: 包含搜索路径、Glob 模式和最大结果数的工具参数
///
/// 返回:
/// - 文件查找结果 JSON
async fn glob_files(args: Value) -> Result<String> {
    glob_files_with_program(args, "rg").await
}

/// 使用指定 ripgrep 程序查找文件，程序不存在时使用内置实现。
///
/// 参数:
/// - `args`: 工具调用参数
/// - `program`: ripgrep 程序名或路径
///
/// 返回:
/// - 文件查找结果 JSON
async fn glob_files_with_program(args: Value, program: &str) -> Result<String> {
    let path = optional_path(&args).unwrap_or(crate::runtime_cwd::current_dir()?);
    let search_path = prepare_search_path(&path)?;
    let pattern = required(&args, "pattern")?;
    let max_results = max_results(&args);
    let mut command = Command::new(program);
    command
        .arg("--no-config")
        .arg("--files")
        .arg("--no-messages")
        .arg("--hidden")
        .arg(format!("--iglob={pattern}"))
        .args(search_exclude_args(&search_path))
        .arg(".")
        .current_dir(&search_path)
        .stdin(Stdio::null());
    // 1. 优先使用 ripgrep 保持大型目录搜索性能
    if let Some(output) = run_search_command(command).await? {
        return search_output_limited(output, max_results);
    }
    // 2. Windows 发布环境缺少 ripgrep 时使用内置文件遍历
    let result = super::native_search::glob_files(&search_path, &pattern, max_results)?;
    native_search_output(result, max_results)
}

/// 按正则表达式搜索文件内容。
///
/// 参数:
/// - `args`: 包含搜索路径、正则表达式、文件过滤和最大结果数的工具参数
///
/// 返回:
/// - 文本搜索结果 JSON
async fn grep_text(args: Value) -> Result<String> {
    grep_text_with_program(args, "rg").await
}

/// 使用指定 ripgrep 程序搜索文本，程序不存在时使用内置实现。
///
/// 参数:
/// - `args`: 工具调用参数
/// - `program`: ripgrep 程序名或路径
///
/// 返回:
/// - 文本搜索结果 JSON
async fn grep_text_with_program(args: Value, program: &str) -> Result<String> {
    let path = optional_path(&args).unwrap_or(crate::runtime_cwd::current_dir()?);
    let is_file = path.is_file();
    let search_root = if is_file {
        path.parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    } else {
        path.clone()
    };
    let search_root = prepare_search_path(&search_root)?;
    let pattern = required(&args, "pattern")?;
    let max_results = max_results(&args);
    let include = args
        .get("include")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::trim);
    let mut command = Command::new(program);
    command
        .arg("--no-config")
        .arg("--line-number")
        .arg("--no-messages")
        .arg("--hidden")
        .args(search_exclude_args(&search_root))
        .arg(&pattern);
    if let Some(include) = include {
        command.arg("--iglob").arg(include);
    }
    if is_file {
        if let Some(name) = path.file_name() {
            command.arg(name);
        }
    } else {
        command.arg(".");
    }
    command.current_dir(&search_root).stdin(Stdio::null());
    // 1. 优先使用 ripgrep 保持正则搜索性能与兼容性
    if let Some(output) = run_search_command(command).await? {
        return search_output_limited(output, max_results);
    }
    // 2. Windows 发布环境缺少 ripgrep 时使用内置正则搜索
    let result = super::native_search::grep_text(
        &search_root,
        is_file.then_some(path.as_path()),
        &pattern,
        include,
        max_results,
    )?;
    native_search_output(result, max_results)
}

/// 执行搜索命令，程序不存在时返回空以触发内置回退。
///
/// 参数:
/// - `command`: 已完成参数配置的搜索命令
///
/// 返回:
/// - ripgrep 输出；程序不存在时返回空
async fn run_search_command(mut command: Command) -> Result<Option<Output>> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(SEARCH_TIMEOUT_SECONDS),
        command.output(),
    )
    .await
    {
        Ok(Ok(output)) => Ok(Some(output)),
        Ok(Err(err)) if err.kind() == ErrorKind::NotFound => Ok(None),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => bail!("search timed out after {SEARCH_TIMEOUT_SECONDS}s"),
    }
}

/// 将内置搜索结果转换为工具统一 JSON 输出。
///
/// 参数:
/// - `result`: 内置搜索结果
/// - `max_results`: 最大结果数
///
/// 返回:
/// - 工具结果 JSON
fn native_search_output(
    result: super::native_search::NativeSearchResult,
    max_results: usize,
) -> Result<String> {
    let stdout = result.lines.join("\n");
    Ok(serde_json::to_string_pretty(&json!({
        "success": true,
        "exit_code": 0,
        "stdout": clip_output(&stdout),
        "stderr": "",
        "truncated": result.truncated,
        "max_results": max_results,
        "matches": result.lines.len(),
        "note": if result.lines.is_empty() { "no matches" } else { "native fallback" }
    }))?)
}

fn command_output_limited(output: std::process::Output, max_lines: usize) -> Result<String> {
    let stdout_raw = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout_raw
        .lines()
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n");
    let stderr = clip_output(&String::from_utf8_lossy(&output.stderr));
    let truncated = stdout_raw.lines().nth(max_lines).is_some();
    Ok(serde_json::to_string_pretty(&json!({
        "success": output.status.success(),
        "exit_code": output.status.code(),
        "stdout": clip_output(&stdout),
        "stderr": stderr,
        "truncated": truncated,
        "max_results": max_lines
    }))?)
}

fn search_output_limited(output: std::process::Output, max_lines: usize) -> Result<String> {
    if output.status.code() == Some(1) && output.stdout.is_empty() {
        return Ok(serde_json::to_string_pretty(&json!({
            "success": true,
            "exit_code": 0,
            "stdout": "",
            "stderr": clip_output(&String::from_utf8_lossy(&output.stderr)),
            "truncated": false,
            "max_results": max_lines,
            "matches": 0,
            "note": "no matches"
        }))?);
    }
    command_output_limited(output, max_lines)
}

fn prepare_search_path(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if path == Path::new("/usr") || path == Path::new("/var") || path == Path::new("/etc") {
        bail!(
            "refusing broad system search path: {}; use / for protected global search or choose a specific subdirectory",
            path.display()
        );
    }
    Ok(path)
}

fn search_exclude_args(search_root: &Path) -> Vec<String> {
    let mut args = vec!["--glob=!**/.git/**".to_string()];
    if search_root == Path::new("/") {
        args.extend(
            [
                "--glob=!dev/**",
                "--glob=!proc/**",
                "--glob=!sys/**",
                "--glob=!run/**",
                "--glob=!tmp/**",
                "--glob=!var/cache/**",
                "--glob=!var/lib/**",
                "--glob=!var/log/**",
                "--glob=!usr/**",
                "--glob=!nix/**",
                "--glob=!snap/**",
                "--glob=!flatpak/**",
            ]
            .into_iter()
            .map(ToString::to_string),
        );
    }
    args
}

fn max_results(args: &Value) -> usize {
    args.get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .clamp(1, 500) as usize
}

fn clip_output(value: &str) -> String {
    let value = value.trim();
    if value.chars().count() <= MAX_COMMAND_OUTPUT_CHARS {
        value.to_string()
    } else {
        format!(
            "{}\n...[{} {MAX_COMMAND_OUTPUT_CHARS} {}]",
            value
                .chars()
                .take(MAX_COMMAND_OUTPUT_CHARS)
                .collect::<String>(),
            t("truncated to", "已截断到"),
            t("chars", "字符")
        )
    }
}

fn optional_path(args: &Value) -> Option<PathBuf> {
    args.get("path")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(expand_path)
}

fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn required(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {key}", t("required argument missing", "缺少必需参数"))
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn glob_files_matches_filename_case_insensitively() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("ai测试题.txt");
        std::fs::write(&path, "content").unwrap();
        let result = glob_files(json!({
            "path": temp.path().display().to_string(),
            "pattern": "*Ai*测试*",
        }))
        .await
        .unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["success"], true);
        assert!(data["stdout"].as_str().unwrap().contains("ai测试题.txt"));
    }

    #[tokio::test]
    async fn grep_no_matches_is_successful_empty_result() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        std::fs::write(temp.path().join("sample.txt"), "hello").unwrap();
        let result = grep_text(json!({
            "path": temp.path().display().to_string(),
            "pattern": "definitely-not-present",
        }))
        .await
        .unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["success"], true);
        assert_eq!(data["exit_code"], 0);
        assert_eq!(data["stdout"], "");
        assert_eq!(data["note"], "no matches");
    }

    /// 验证缺少 ripgrep 时文本搜索使用内置回退实现。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn grep_falls_back_when_ripgrep_is_missing() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        std::fs::write(temp.path().join("sample.txt"), "first\nneedle\nthird\n").unwrap();

        let result = grep_text_with_program(
            json!({
                "path": temp.path().display().to_string(),
                "pattern": "needle",
            }),
            "sai-missing-rg",
        )
        .await
        .unwrap();

        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["success"], true);
        assert!(data["stdout"]
            .as_str()
            .unwrap()
            .contains("sample.txt:2:needle"));
    }

    /// 验证缺少 ripgrep 时文件查找使用内置回退实现。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn glob_falls_back_when_ripgrep_is_missing() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        std::fs::write(temp.path().join("Example.RS"), "content").unwrap();

        let result = glob_files_with_program(
            json!({
                "path": temp.path().display().to_string(),
                "pattern": "*.rs",
            }),
            "sai-missing-rg",
        )
        .await
        .unwrap();

        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["success"], true);
        assert!(data["stdout"].as_str().unwrap().contains("Example.RS"));
    }

    #[test]
    fn root_search_uses_protective_excludes() {
        let root = Path::new("/");
        assert!(prepare_search_path(root).is_ok());
        let args = search_exclude_args(root).join(" ");
        assert!(args.contains("--glob=!proc/**"));
        assert!(args.contains("--glob=!usr/**"));
    }
}
