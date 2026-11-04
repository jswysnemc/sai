//! 【命令预览】【Windows 路径回归】验证路径之后的参数着色和折行原文。

use super::*;
use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};
use crate::render::render_width::with_render_width;
use crate::render::style::{CODE_STRING_STYLE, PRIMARY_STYLE};

/// 【命令预览】【真实命令】未加引号的 Windows 路径不能按 shell 字符转义着色。
/// 参数: 无；返回: 无，反斜杠误着色或后续参数失色时断言失败。
#[test]
fn windows_command_path_does_not_turn_separators_into_string_escapes() {
    let command = r"git -C D:\Code\Systemui-dev log -n 5 --oneline src/com/android/systemui/keyguard/ui/viewmodel/DeviceEntryIconViewModel";
    let args = serde_json::json!({"command": command, "_sai_command_shell": "pwsh"}).to_string();
    let output = with_render_width(200, || {
        render_command_block_with_action(&args, "Ran", ToolHealth::Ok)
    });
    assert!(
        !output.contains(&format!("{CODE_STRING_STYLE}\\C")),
        "{output:?}"
    );
    assert!(
        !output.contains(&format!("{CODE_STRING_STYLE}\\S")),
        "{output:?}"
    );
    assert!(
        output.contains(&format!("{PRIMARY_STYLE}log")),
        "{output:?}"
    );
    assert!(strip_ansi(&output).contains(command), "{output:?}");
}

/// 【命令预览】【折行边界】各种终端宽度都保留最后的 n 和路径原文。
/// 参数: 无；返回: 无，预览遗失字符或超出行宽时断言失败。
#[test]
fn windows_command_preview_preserves_tail_at_wrap_boundaries() {
    let command = r"git -C D:\Code\Systemui-dev log -n 5 --oneline src/com/android/systemui/keyguard/ui/viewmodel/OffToLockscreenTransition";
    let args = serde_json::json!({"command": command, "_sai_command_shell": "pwsh"}).to_string();
    for width in 40..=160 {
        let output = with_render_width(width, || {
            render_command_block_with_action(&args, "Ran", ToolHealth::Ok)
        });
        let mut source = String::new();
        for (row, line) in output.lines().enumerate() {
            assert!(ansi_visible_width(line) <= width, "width={width} {line:?}");
            let plain = strip_ansi(line);
            source.push_str(if row == 0 {
                plain.strip_prefix("• Ran $ ").unwrap()
            } else {
                &plain[8..]
            });
        }
        assert_eq!(source, command, "width={width} output={output:?}");
    }
}

/// 【命令预览】【脚本换行】真实注释只影响本行，路径闭合引号及后续代码保持完整。
/// 参数: 无；返回: 无，原文丢失或下一逻辑行仍为注释色时断言失败。
#[test]
fn powershell_command_preview_ends_comments_at_logical_newlines() {
    let command = r#"$extractDir = "D:\work\11月 \11.4-DEPLOYC-16124-锁屏点想帮帮无反应\attachments\extracted"
$selected = @()
$folded = @()

foreach ($f in $allFiles) {
    # 排除 dropbox 碎片与 logcats 分卷
    $rel = $f.FullName.Substring($extractDir.Length + 1)"#;
    let args = serde_json::json!({"command": command, "_sai_command_shell": "pwsh"}).to_string();
    let output = with_render_width(200, || {
        crate::render::render_expand::with_expanded_render(|| {
            render_command_block_with_action(&args, "Ran", ToolHealth::Ok)
        })
    });
    assert!(strip_ansi(&output).contains("\\extracted\""), "{output:?}");
    let last = output.lines().last().unwrap();
    assert!(last.contains(&format!("{PRIMARY_STYLE}rel")), "{last:?}");
    assert!(
        !last.contains(crate::render::style::CODE_COMMENT_STYLE),
        "{last:?}"
    );
}

/// 【命令预览】【参数流转义】JSON 对象未闭合时仍正确解码真实路径中的中文。
/// 参数: 无；返回: 无，Unicode 转义变成字面量并挤掉闭合引号时断言失败。
#[test]
fn streamed_windows_path_decodes_unicode_without_changing_the_quote() {
    let arguments = r#"{"_sai_command_shell":"pwsh", "command":"$extractDir = \"D:\\work\\11\u6708 \\11.4-DEPLOYC-16124-\u9501\u5c4f\u70b9\u60f3\u5e2e\u5e2e\u65e0\u53cd\u5e94\\attachments\\extracted\"""#;
    let output = with_render_width(200, || {
        render_command_block_with_action(arguments, "Ran", ToolHealth::Ok)
    });
    let plain = strip_ansi(&output);
    assert!(
        plain.contains(
            r#""D:\work\11月 \11.4-DEPLOYC-16124-锁屏点想帮帮无反应\attachments\extracted""#
        ),
        "{plain:?}"
    );
}
