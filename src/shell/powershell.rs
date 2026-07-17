use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
#[cfg(windows)]
use directories::UserDirs;
use std::io::Write;
use std::path::{Path, PathBuf};

const BEGIN_MARKER: &str = "# >>> sai powershell hook >>>";
const END_MARKER: &str = "# <<< sai powershell hook <<<";

/// 生成 PowerShell 命令未找到时的自然语言拦截 hook。
///
/// 返回:
/// - PowerShell hook 脚本文本
pub fn hook() -> &'static str {
    r#"$script:SaiCommandNotFoundActionInstalled = $script:SaiCommandNotFoundActionInstalled -eq $true
if (-not $script:SaiCommandNotFoundActionInstalled) {
    $script:SaiPreviousCommandNotFoundAction = $ExecutionContext.InvokeCommand.CommandNotFoundAction
    $script:SaiCommandNotFoundActionInstalled = $true
}

function Invoke-SaiPreviousCommandNotFoundAction {
    param($Name, $EventArgs)

    if ($script:SaiPreviousCommandNotFoundAction) {
        & $script:SaiPreviousCommandNotFoundAction $Name $EventArgs
    }
}

function Get-SaiCommandLineCandidate {
    param([string] $Name)

    $fallback = [string]$Name
    try {
        $historyItems = [Microsoft.PowerShell.PSConsoleReadLine]::GetHistoryItems()
        if ($historyItems.Count -eq 0) {
            return $fallback
        }

        $candidate = [string]$historyItems[$historyItems.Count - 1].CommandLine
        $pattern = '^\s*' + [regex]::Escape($Name) + '(\s|$)'
        if ($candidate -match $pattern) {
            return $candidate.Trim()
        }
    } catch {
    }

    return $fallback
}

function Test-SaiNaturalLanguageCommand {
    param([string] $Text)

    if ([string]::IsNullOrWhiteSpace($Text)) {
        return $false
    }

    $trimmed = $Text.Trim()
    if ($trimmed.Length -gt 120) {
        return $false
    }
    if ($trimmed -match "[`r`n]") {
        return $false
    }
    if ($trimmed -match '^\s*([-#./~0-9<]|[0-9]+[.)])') {
        return $false
    }
    if ($trimmed -match '[/\\=|;&<>$`(){}\[\]*]') {
        return $false
    }

    $parts = @($trimmed -split '\s+' | Where-Object { $_ -ne '' })
    return (($trimmed -match '[^\x00-\x7F]') -or ($parts.Count -gt 1))
}

$ExecutionContext.InvokeCommand.CommandNotFoundAction = {
    param($Name, $EventArgs)

    $text = Get-SaiCommandLineCandidate -Name ([string]$Name)
    if (-not (Test-SaiNaturalLanguageCommand -Text $text)) {
        Invoke-SaiPreviousCommandNotFoundAction $Name $EventArgs
        return
    }

    $saiMessage = $text
    $EventArgs.CommandScriptBlock = {
        sai --shell-intercept --shell powershell -- $saiMessage 2>$null
    }.GetNewClosure()
}
"#
}

/// 安装 PowerShell hook 并写入用户 profile。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 安装结果
pub fn install(paths: &SaiPaths) -> Result<()> {
    if let Some(parent) = paths.powershell_hook_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&paths.powershell_hook_file, hook())?;
    let profile_paths = profile_paths();
    for profile_path in &profile_paths {
        append_source_block(
            profile_path,
            BEGIN_MARKER,
            END_MARKER,
            &paths.powershell_hook_file,
        )?;
    }
    println!(
        "{}: {}",
        t("installed PowerShell hook", "已安装 PowerShell hook"),
        paths.powershell_hook_file.display()
    );
    for profile_path in profile_paths {
        println!("{}: {}", t("updated", "已更新"), profile_path.display());
    }
    super::print_reload_hint("powershell", &paths.powershell_hook_file);
    Ok(())
}

/// 卸载 PowerShell hook 并从用户 profile 移除加载块。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 卸载结果
pub fn uninstall(paths: &SaiPaths) -> Result<()> {
    remove_file_if_exists(&paths.powershell_hook_file)?;
    for profile_path in profile_paths() {
        remove_source_block(&profile_path, BEGIN_MARKER, END_MARKER)?;
    }
    println!(
        "{}: powershell",
        t("removed Sai shell hook", "已移除 Sai shell hook")
    );
    Ok(())
}

/// 获取当前用户常见 PowerShell profile 路径。
///
/// 返回:
/// - profile 文件路径列表
#[cfg(windows)]
fn profile_paths() -> Vec<PathBuf> {
    let documents_dir = UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(PathBuf::from))
        .unwrap_or_else(|| home_dir().join("Documents"));
    vec![
        documents_dir.join("PowerShell/profile.ps1"),
        documents_dir.join("WindowsPowerShell/profile.ps1"),
    ]
}

/// 获取当前用户常见 PowerShell profile 路径。
///
/// 返回:
/// - profile 文件路径列表
#[cfg(not(windows))]
fn profile_paths() -> Vec<PathBuf> {
    vec![home_dir().join(".config/powershell/profile.ps1")]
}

/// 获取用户主目录路径。
///
/// 返回:
/// - 用户主目录路径，无法获取时返回相对路径
fn home_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 向 profile 追加 hook 加载块。
///
/// 参数:
/// - `profile_path`: profile 文件路径
/// - `begin`: 开始标记
/// - `end`: 结束标记
/// - `hook_file`: hook 文件路径
///
/// 返回:
/// - 写入结果
fn append_source_block(
    profile_path: &Path,
    begin: &str,
    end: &str,
    hook_file: &Path,
) -> Result<()> {
    let existing = std::fs::read_to_string(profile_path).unwrap_or_default();
    if existing.contains(begin) && existing.contains(end) {
        return Ok(());
    }
    if let Some(parent) = profile_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(profile_path)?;
    if !existing.ends_with('\n') && !existing.is_empty() {
        writeln!(file)?;
    }
    writeln!(file, "{begin}")?;
    writeln!(
        file,
        "if (Test-Path -LiteralPath {}) {{",
        powershell_quote(hook_file)
    )?;
    writeln!(file, "    . {}", powershell_quote(hook_file))?;
    writeln!(file, "}}")?;
    writeln!(file, "{end}")?;
    Ok(())
}

/// 从 profile 移除 hook 加载块。
///
/// 参数:
/// - `profile_path`: profile 文件路径
/// - `begin`: 开始标记
/// - `end`: 结束标记
///
/// 返回:
/// - 移除结果
fn remove_source_block(profile_path: &Path, begin: &str, end: &str) -> Result<()> {
    let Ok(existing) = std::fs::read_to_string(profile_path) else {
        return Ok(());
    };
    let Some(begin_index) = existing.find(begin) else {
        return Ok(());
    };
    let Some(end_relative) = existing[begin_index..].find(end) else {
        return Ok(());
    };
    let mut end_index = begin_index + end_relative + end.len();
    if existing.as_bytes().get(end_index) == Some(&b'\r') {
        end_index += 1;
    }
    if existing.as_bytes().get(end_index) == Some(&b'\n') {
        end_index += 1;
    }
    let mut updated = String::new();
    updated.push_str(&existing[..begin_index]);
    updated.push_str(&existing[end_index..]);
    std::fs::write(profile_path, updated)?;
    Ok(())
}

/// 删除指定文件，文件不存在时视为成功。
///
/// 参数:
/// - `path`: 待删除文件路径
///
/// 返回:
/// - 删除结果
fn remove_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// 将路径转换为 PowerShell 单引号字符串。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - PowerShell 字符串字面量
fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_hook_registers_command_not_found_action() {
        let hook = hook();
        assert!(hook.contains("CommandNotFoundAction"));
        assert!(hook.contains("--shell powershell"));
        assert!(hook.contains("CommandScriptBlock"));
        assert!(hook.contains("GetHistoryItems"));
    }

    #[test]
    fn powershell_quote_escapes_single_quote() {
        let quoted = powershell_quote(Path::new("a'b.ps1"));
        assert_eq!(quoted, "'a''b.ps1'");
    }
}
