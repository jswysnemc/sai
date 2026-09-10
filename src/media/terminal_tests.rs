use std::path::Path;
use std::process::{Command, Output};

const CHILD: &str = "media::terminal::tests::render_without_publish_child";

/// 【终端图片测试】【独立进程】捕获真实标准输出，避免污染其他测试的终端协议环境。
/// @param protocol 指定 Kitty 或 iTerm；destination 为渲染内容保存位置
/// @returns 子进程的完整标准输出与退出状态
fn render_in_child(protocol: &str, destination: &Path) -> Output {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD, "--ignored", "--nocapture"])
        .env("SAI_IMAGE_RENDER_TEST_OUTPUT", destination)
        .env_remove("KITTY_WINDOW_ID")
        .env_remove("TMUX")
        .env_remove("WT_SESSION")
        .env("TERM", "xterm-256color")
        .env(
            "TERM_PROGRAM",
            if protocol == "kitty" {
                "ghostty"
            } else {
                "iTerm.app"
            },
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "render child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// 【终端图片测试】【延后输出】丢弃未提交的渲染结果时，真实标准输出不能含图片数据。
#[test]
fn terminal_rendering_stays_buffered_until_published() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("rendered.txt");
    let output = render_in_child("kitty", &path);
    assert!(
        !output.stdout.windows(3).any(|bytes| bytes == b"\x1b_G"),
        "rendering wrote Kitty protocol data before publishing"
    );
    let rendered = std::fs::read_to_string(path).unwrap();
    let transmission = rendered.find("\x1b_Ga=t").expect("buffered image data");
    let placement = rendered.find("\x1b_Ga=p").expect("buffered placement");
    assert!(transmission < placement);
    assert_eq!(rendered.matches("\x1b_Ga=t").count(), 1);
}

/// 【终端图片测试】【显示尺寸】两种图片协议都必须遵守调用方的四列两行上限。
#[test]
fn terminal_rendering_respects_graphics_protocol_sizes() {
    let root = tempfile::tempdir().unwrap();
    for protocol in ["kitty", "iterm"] {
        let path = root.path().join(format!("{protocol}.txt"));
        render_in_child(protocol, &path);
        let rendered = std::fs::read_to_string(path).unwrap();
        let expected = if protocol == "kitty" {
            ",c=4,r=1"
        } else {
            ";width=4;height=1;"
        };
        assert!(
            rendered.contains(expected),
            "{protocol} renderer ignored the requested cell limits"
        );
    }
}

/// 【终端图片测试】【渲染子进程】生成固定图片并丢弃首次结果，再保存第二次完整渲染内容。
/// @returns 无；父测试检查实际输出和保存文件，重复渲染不能遗漏图片数据
#[test]
#[ignore = "isolated child process driven by the terminal rendering tests"]
fn render_without_publish_child() {
    let Some(destination) = std::env::var_os("SAI_IMAGE_RENDER_TEST_OUTPUT") else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture.png");
    image::RgbaImage::from_pixel(160, 80, image::Rgba([23, 67, 119, 255]))
        .save(&path)
        .unwrap();
    drop(super::render(&path, Some("4x2")).unwrap());
    let rendered = super::render(&path, Some("4x2")).unwrap();
    std::fs::write(destination, rendered).unwrap();
}
