use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use std::io::Write;
use std::path::Path;

const BEGIN_MARKER: &str = "# >>> sai zsh hook >>>";
const END_MARKER: &str = "# <<< sai zsh hook <<<";

pub fn hook() -> &'static str {
    r#"autoload -Uz add-zsh-hook

_sai_store_preexec_command() {
    [[ -o interactive ]] || return 0
    local text="$1"
    [[ -n "$text" ]] || return 0
    [[ "$text" != sai && "$text" != sai\ * ]] || return 0
    sai --shell-intercept --shell zsh -- "$text" >/dev/null 2>&1
    return 0
}

add-zsh-hook -d preexec _sai_store_preexec_command 2>/dev/null
add-zsh-hook preexec _sai_store_preexec_command
"#
}

pub fn install(paths: &SaiPaths) -> Result<()> {
    if let Some(parent) = paths.zsh_hook_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&paths.zsh_hook_file, hook())?;
    let rc_path = home_file(".zshrc");
    append_source_block(&rc_path, BEGIN_MARKER, END_MARKER, &paths.zsh_hook_file)?;
    println!(
        "{}: {}",
        t("installed zsh hook", "已安装 zsh hook"),
        paths.zsh_hook_file.display()
    );
    println!("{}: {}", t("updated", "已更新"), rc_path.display());
    super::print_reload_hint("zsh", &paths.zsh_hook_file);
    Ok(())
}

pub fn uninstall(paths: &SaiPaths) -> Result<bool> {
    let removed_file = remove_file_if_exists(&paths.zsh_hook_file)?;
    let rc_path = home_file(".zshrc");
    let removed_block = remove_source_block(&rc_path, BEGIN_MARKER, END_MARKER)?;
    let removed = removed_file || removed_block;
    if removed {
        println!(
            "{}: zsh",
            t("removed Sai shell hook", "已移除 Sai shell hook")
        );
    }
    Ok(removed)
}

fn home_file(name: &str) -> std::path::PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(name))
        .unwrap_or_else(|| std::path::PathBuf::from(name))
}

fn append_source_block(rc_path: &Path, begin: &str, end: &str, hook_file: &Path) -> Result<()> {
    let existing = std::fs::read_to_string(rc_path).unwrap_or_default();
    if existing.contains(begin) && existing.contains(end) {
        return Ok(());
    }
    if let Some(parent) = rc_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc_path)?;
    if !existing.ends_with('\n') && !existing.is_empty() {
        writeln!(file)?;
    }
    writeln!(file, "{begin}")?;
    writeln!(file, "[ -r {:?} ] && source {:?}", hook_file, hook_file)?;
    writeln!(file, "{end}")?;
    Ok(())
}

fn remove_source_block(rc_path: &Path, begin: &str, end: &str) -> Result<bool> {
    let Ok(existing) = std::fs::read_to_string(rc_path) else {
        return Ok(false);
    };
    let Some(begin_index) = existing.find(begin) else {
        return Ok(false);
    };
    let Some(end_relative) = existing[begin_index..].find(end) else {
        return Ok(false);
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
    std::fs::write(rc_path, updated)?;
    Ok(true)
}

fn remove_file_if_exists(path: &Path) -> Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_hook_stores_commands_before_execution() {
        let hook = hook();
        assert!(hook.contains("add-zsh-hook preexec"));
        assert!(hook.contains("--shell zsh"));
        assert!(hook.contains("\"$text\""));
        assert!(hook.contains("return 0"));
        assert!(hook.contains("$text\" != sai"));
    }

    #[test]
    fn zsh_hook_does_not_block_shell_commands() {
        let hook = hook();
        assert!(!hook.contains("command_not_found_handler"));
        assert!(!hook.contains("return 127"));
    }

    #[test]
    fn remove_file_if_exists_reports_whether_file_was_removed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("hook.zsh");

        assert!(!remove_file_if_exists(&path).unwrap());
        std::fs::write(&path, hook()).unwrap();
        assert!(remove_file_if_exists(&path).unwrap());
        assert!(!remove_file_if_exists(&path).unwrap());
    }

    #[test]
    fn remove_source_block_reports_whether_block_was_removed() {
        let temp = tempfile::tempdir().unwrap();
        let rc_path = temp.path().join(".zshrc");
        std::fs::write(
            &rc_path,
            format!("before\n{BEGIN_MARKER}\nsource hook\n{END_MARKER}\nafter\n"),
        )
        .unwrap();

        assert!(remove_source_block(&rc_path, BEGIN_MARKER, END_MARKER).unwrap());
        assert_eq!(
            std::fs::read_to_string(&rc_path).unwrap(),
            "before\nafter\n"
        );
        assert!(!remove_source_block(&rc_path, BEGIN_MARKER, END_MARKER).unwrap());
    }
}
