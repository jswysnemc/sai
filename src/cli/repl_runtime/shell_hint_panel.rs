use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use std::io::Write;
use std::path::Path;

/// 输入以 `!` 开头时展示的本地 shell 提示面板。
///
/// 对齐参考：输入区幽灵说明 + 下方展示当前模型与工作目录，无实心黑底。
pub(super) struct ShellHintPanel {
    visible: bool,
    model: String,
    directory: String,
}

impl ShellHintPanel {
    /// 根据当前输入与 chrome 构造 shell 提示面板。
    ///
    /// 参数:
    /// - `input`: 当前输入
    /// - `model`: 当前模型名
    /// - `directory`: 当前工作目录绝对路径
    ///
    /// 返回:
    /// - shell 提示面板
    pub(super) fn new(input: &str, model: &str, directory: &str) -> Self {
        Self {
            visible: input.starts_with('!'),
            model: model.to_string(),
            directory: directory.to_string(),
        }
    }

    /// 判断面板是否需要展示。
    ///
    /// 返回:
    /// - 输入以 `!` 开头时为 true
    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    /// 返回面板占用行数。
    ///
    /// 返回:
    /// - 提示行 + 模型行 + 目录行
    pub(super) fn height(&self) -> u16 {
        if self.visible {
            3
        } else {
            0
        }
    }

    /// 返回面板各行文本（供签名与测试）。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 面板行
    pub(super) fn rendered_lines(&self, cols: usize) -> Vec<String> {
        if !self.visible {
            return Vec::new();
        }
        let hint = t("Run a command – e.g., ls", "运行命令 – 例如 ls");
        let cwd = display_cwd(&self.directory);
        vec![
            truncate_ansi(&format!("\x1b[2m  {hint}\x1b[0m"), cols),
            truncate_ansi(&format!("\x1b[38;5;81m  {}\x1b[0m", self.model), cols),
            truncate_ansi(&format!("\x1b[2m  {cwd}\x1b[0m"), cols),
        ]
    }

    /// 在输入框下方绘制 shell 提示面板。
    ///
    /// 参数:
    /// - `output`: 终端输出
    /// - `top`: 面板顶部行号
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 绘制是否成功
    pub(super) fn draw<W: Write>(&self, output: &mut W, top: u16, cols: usize) -> Result<()> {
        for (index, line) in self.rendered_lines(cols).into_iter().enumerate() {
            queue!(
                output,
                MoveTo(0, top.saturating_add(index as u16)),
                Print(line)
            )?;
        }
        Ok(())
    }
}

/// 仅输入 `!` 时附在输入行后的幽灵提示（不计入光标宽度）。
///
/// 参数:
/// - `input`: 当前输入
///
/// 返回:
/// - 幽灵提示 ANSI；不需要时返回空
pub(super) fn bang_ghost_suffix(input: &str) -> Option<&'static str> {
    if input == "!" {
        Some(t(
            " Run a command – e.g., ls",
            " 运行命令 – 例如 ls",
        ))
    } else {
        None
    }
}

/// 将绝对路径压缩为 `~` 相对形式。
///
/// 参数:
/// - `path`: 绝对或相对路径
///
/// 返回:
/// - 展示用路径
fn display_cwd(path: &str) -> String {
    // Windows 没有 HOME，用户目录在 USERPROFILE
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok();
    display_cwd_with_home(path, home.as_deref())
}

/// 以显式家目录将路径压缩为 `~` 形式（纯函数，便于跨平台测试）。
///
/// 参数:
/// - `path`: 绝对或相对路径
/// - `home`: 家目录；缺失时原样返回
///
/// 返回:
/// - 展示用路径
fn display_cwd_with_home(path: &str, home: Option<&str>) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "~".to_string();
    }
    let Some(home) = home.map(str::trim).filter(|value| !value.is_empty()) else {
        return trimmed.to_string();
    };
    // Windows 路径分隔符统一为正斜杠再比较
    let home_norm = home.replace('\\', "/");
    let trimmed_norm = trimmed.replace('\\', "/");
    if trimmed_norm == home_norm {
        return "~".to_string();
    }
    let home_path = Path::new(&home_norm);
    let current = Path::new(&trimmed_norm);
    if let Ok(relative) = current.strip_prefix(home_path) {
        let rest = relative.to_string_lossy();
        if rest.is_empty() {
            "~".to_string()
        } else {
            format!("~/{rest}")
        }
    } else {
        trimmed.to_string()
    }
}

/// 按显示宽度截断已含 ANSI 的单行。
///
/// 参数:
/// - `line`: ANSI 行
/// - `cols`: 最大列数
///
/// 返回:
/// - 截断后的行
fn truncate_ansi(line: &str, cols: usize) -> String {
    let plain = crate::render::activity_animation::strip_ansi_for_test(line);
    if visible_width(&plain) <= cols {
        return line.to_string();
    }
    // 超宽时退回纯文本截断，避免半截转义序列
    let mut output = String::new();
    let mut used = 0usize;
    for ch in plain.chars() {
        let w = visible_width(&ch.to_string());
        if used + w > cols.saturating_sub(1) {
            break;
        }
        output.push(ch);
        used += w;
    }
    output.push('…');
    format!("\x1b[2m{output}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_only_for_bang_prefix() {
        assert!(!ShellHintPanel::new("hello", "m", "/tmp").is_visible());
        assert!(ShellHintPanel::new("!", "m", "/tmp").is_visible());
        assert!(ShellHintPanel::new("!ls", "m", "/tmp").is_visible());
    }

    #[test]
    fn ghost_suffix_only_for_bare_bang() {
        assert!(bang_ghost_suffix("!").is_some());
        assert!(bang_ghost_suffix("!ls").is_none());
        assert!(bang_ghost_suffix("").is_none());
    }

    #[test]
    fn home_directory_renders_as_tilde() {
        // 纯函数直接注入家目录，不依赖 HOME/USERPROFILE 环境（Windows CI 无 HOME）
        assert_eq!(display_cwd_with_home("/home/user", Some("/home/user")), "~");
        assert_eq!(
            display_cwd_with_home("/home/user/proj", Some("/home/user")),
            "~/proj"
        );
        // Windows 风格路径同样压缩
        assert_eq!(
            display_cwd_with_home("C:\\Users\\dev\\proj", Some("C:\\Users\\dev")),
            "~/proj"
        );
        // 无家目录时原样返回
        assert_eq!(display_cwd_with_home("/srv/data", None), "/srv/data");
    }

    #[test]
    fn panel_lists_hint_model_and_cwd() {
        let panel = ShellHintPanel::new("!", "gpt-test", "/tmp");
        let lines = panel.rendered_lines(80);
        assert_eq!(lines.len(), 3);
        let plain: Vec<_> = lines
            .iter()
            .map(|line| crate::render::activity_animation::strip_ansi_for_test(line))
            .collect();
        assert!(plain[0].contains("ls") || plain[0].contains("命令"));
        assert!(plain[1].contains("gpt-test"));
        assert!(plain[2].contains("/tmp") || plain[2].contains('~'));
    }
}
