//! 【代码着色】【Windows 路径回归】验证真实脚本原文和行末样式不受路径转义影响。

use super::*;
use crate::render::ansi_clip::strip_ansi;

const SCRIPT: &str = r#"$extractDir = "D:\work\11月 \11.4-DEPLOYC-16124-锁屏点想帮帮无反应\attachments\extracted"
$selected = @()
$folded = @()

foreach ($f in $allFiles) {
    # 排除 dropbox 碎片与 logcats 分卷
    $rel = $f.FullName.Substring($extractDir.Length + 1)"#;

/// 【代码着色】【用户脚本】原始路径及闭合引号完整保留，注释之后的代码仍独立着色。
/// 参数: 无；返回: 无，原文丢失或颜色停止时断言失败。
#[test]
fn powershell_windows_path_preserves_quotes_and_following_colors() {
    for lang in ["powershell", "ps1", "pwsh"] {
        for line in SCRIPT.lines() {
            let rendered = highlight_code_line(lang, line);
            assert_eq!(
                strip_ansi(&rendered),
                line,
                "lang={lang} rendered={rendered:?}"
            );
            if line.contains("# 排除") {
                assert!(
                    rendered.contains(CODE_COMMENT_STYLE),
                    "lang={lang} {rendered:?}"
                );
            }
            if line.contains("$rel =") {
                assert!(rendered.contains(PRIMARY_STYLE), "lang={lang} {rendered:?}");
                assert!(
                    !rendered.starts_with(CODE_COMMENT_STYLE),
                    "lang={lang} {rendered:?}"
                );
            }
        }
    }
}

/// 【代码着色】【行末字符】命令路径和文件名尾部必须全部保留。
/// 参数: 无；返回: 无，着色切片丢弃最后字符时断言失败。
#[test]
fn shell_windows_path_preserves_the_last_filename_character() {
    for line in [
        r"git -C D:\Code\Systemui-dev log -n 5 --oneline src/com/android/systemui/keyguard/ui/viewmodel/DeviceEntryIconViewModel",
        r"git -C D:\Code\Systemui-dev log -n 5 --oneline src/com/android/systemui/keyguard/ui/viewmodel/OffToLockscreenTransition",
    ] {
        let output = highlight_code_line("sh", line);
        assert_eq!(strip_ansi(&output), line, "rendered={output:?}");
    }
}
