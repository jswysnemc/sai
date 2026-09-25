use super::read_file;
use serde_json::json;

/// 在工作目录下创建隔离临时目录，保证相对路径测试可复现。
fn temp_dir() -> tempfile::TempDir {
    let cwd = crate::runtime_cwd::current_dir().unwrap();
    tempfile::tempdir_in(cwd).unwrap()
}

/// 文本结果按 `行号<TAB>正文` 返回请求窗口。
#[tokio::test]
async fn read_file_returns_numbered_window() {
    let temp = temp_dir();
    let path = temp.path().join("sample.txt");
    std::fs::write(&path, "one\ntwo\nthree\n").unwrap();

    let output = read_file(json!({"path": path.display().to_string(), "offset": 2, "limit": 1}))
        .await
        .unwrap();

    assert_eq!(output.content, "2\ttwo");
    assert!(output.model_attachments.is_empty());
}

/// 未指定范围时读取整个文件。
#[tokio::test]
async fn read_file_reads_whole_file_by_default() {
    let temp = temp_dir();
    let path = temp.path().join("sample.rs");
    std::fs::write(&path, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();

    let output = read_file(json!({"path": path.display().to_string()}))
        .await
        .unwrap();

    assert_eq!(
        output.content,
        "1\tfn main() {\n2\t    println!(\"hi\");\n3\t}"
    );
}

/// 缺失路径的错误带展开后的绝对路径与工作目录说明。
#[tokio::test]
async fn read_file_missing_path_error_includes_expanded_path() {
    let temp = temp_dir();
    let missing = temp.path().join("nowhere.txt");

    let error = read_file(json!({"path": missing.display().to_string()}))
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains(&missing.display().to_string()), "{error}");
    assert!(error.contains("does not exist"), "{error}");
    assert!(error.contains("current working directory"), "{error}");
    assert!(!error.contains("os error"), "{error}");
}

/// 相对路径读取失败时错误展示拼接后的绝对路径。
#[tokio::test]
async fn read_file_relative_path_error_reports_joined_path() {
    let temp = temp_dir();
    let workspace = temp.path().to_path_buf();

    let error = crate::runtime_cwd::scope(workspace.clone(), async {
        read_file(json!({"path": "src/nowhere.rs"}))
            .await
            .unwrap_err()
            .to_string()
    })
    .await;

    assert!(error.contains(&workspace.join("src/nowhere.rs").display().to_string()));
}

/// 二进制扩展名与二进制内容都会被拒绝。
#[tokio::test]
async fn read_file_rejects_binary_files() {
    let temp = temp_dir();
    let archive = temp.path().join("bundle.zip");
    std::fs::write(&archive, [0x50, 0x4b, 0x03, 0x04]).unwrap();
    let blob = temp.path().join("sample.bin2");
    std::fs::write(&blob, [0, 1, 2, 3]).unwrap();

    let by_extension = read_file(json!({"path": archive.display().to_string()}))
        .await
        .unwrap_err()
        .to_string();
    let by_content = read_file(json!({"path": blob.display().to_string()}))
        .await
        .unwrap_err()
        .to_string();

    assert!(
        by_extension.contains("cannot read binary files"),
        "{by_extension}"
    );
    assert!(
        by_content.contains("cannot read binary file"),
        "{by_content}"
    );
}

/// 目录读取返回排序后的列表。
#[tokio::test]
async fn read_file_lists_directories() {
    let temp = temp_dir();
    std::fs::write(temp.path().join("b.txt"), "").unwrap();
    std::fs::create_dir(temp.path().join("a")).unwrap();

    let output = read_file(json!({"path": temp.path().display().to_string()}))
        .await
        .unwrap();

    assert!(
        output.content.contains("Entries 1-2 of 2:\na/\nb.txt"),
        "{}",
        output.content
    );
}

/// 图片直接作为模型附件返回，文本结果不含图片数据。
#[tokio::test]
async fn read_file_attaches_images_for_the_current_model() {
    use image::ImageEncoder;
    let temp = temp_dir();
    let path = temp.path().join("shot.png");
    let rgba = image::RgbaImage::from_pixel(16, 9, image::Rgba([1, 2, 3, 255]));
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(rgba.as_raw(), 16, 9, image::ExtendedColorType::Rgba8)
        .unwrap();
    std::fs::write(&path, bytes).unwrap();

    let output = read_file(json!({"path": path.display().to_string()}))
        .await
        .unwrap();

    assert!(
        output.content.starts_with("[Image: source: "),
        "{}",
        output.content
    );
    assert_eq!(output.model_attachments.len(), 1);
    assert!(!output.content.contains("base64,"));
}

/// 设备文件与非法 pages 在读取前被拦下。
#[tokio::test]
async fn read_file_rejects_devices_and_invalid_pages() {
    let device = read_file(json!({"path": "/dev/zero"}))
        .await
        .unwrap_err()
        .to_string();
    let pages = read_file(json!({"path": "/tmp/doc.pdf", "pages": "0-2"}))
        .await
        .unwrap_err()
        .to_string();

    assert!(
        device.contains("would block or produce infinite output"),
        "{device}"
    );
    assert!(pages.contains("Invalid pages parameter"), "{pages}");
}
