//! 补丁语法和文本匹配的边界回归测试。

use super::{model::Operation, parser::parse, text};

/// 【文件补丁测试】【文本夹具】使用真实解析器构造单文件更新并应用到输入文本。
/// 参数: raw 为原文，body 为补丁片段；返回: 应用结果。
fn update(raw: &str, body: &str) -> anyhow::Result<String> {
    let operations = parse(&format!(
        "*** Begin Patch\n*** Update File: sample.txt\n{body}\n*** End Patch"
    ))?;
    let Operation::Update { hunks, .. } = &operations[0] else {
        unreachable!()
    };
    text::apply(raw, hunks)
}

/// 【文件补丁测试】【协议校验】拒绝空补丁、残缺边界、未知动作和无效上下文。
/// 参数: 无；返回: 无。
#[test]
fn rejects_malformed_and_incomplete_patches() {
    for body in [
        "",
        "*** Begin Patch\n*** End Patch",
        "*** Begin Patch\n*** Add File: a\n+x",
        "*** Begin Patch\n*** Delete File: a\ntrailing\n*** End Patch",
        "*** Begin Patch\n*** Update File: a\n@@\n context\n*** End Patch",
        "*** Begin Patch\n*** Add File: a\nx\n*** End Patch",
        "*** Begin Patch\n*** Add File: a\n+x\n*** End Patch\njunk",
    ] {
        assert!(parse(body).is_err(), "{body:?}");
    }
}

/// 【文件补丁测试】【换行保持】纯 CRLF、BOM、无末尾换行和空文件均保持明确语义。
/// 参数: 无；返回: 无。
#[test]
fn preserves_line_endings_bom_and_final_newline() {
    assert_eq!(
        update("\u{feff}a\r\nb\r\n", "@@\n a\n-b\n+B").unwrap(),
        "\u{feff}a\r\nB\r\n"
    );
    assert_eq!(update("a\nb", "@@\n-b\n+B").unwrap(), "a\nB");
    assert_eq!(update("", "@@\n+first").unwrap(), "first\n");
    assert_eq!(
        update("a\n", "@@\n-a\n+b\n\\ No newline at end of file").unwrap(),
        "b"
    );
    assert_eq!(
        update("a", "@@\n-a\n\\ No newline at end of file\n+a").unwrap(),
        "a\n"
    );
    assert_eq!(update("a\n", "@@\n-a").unwrap(), "");
    assert!(update("a\r\nb\n", "@@\n-b\n+B").is_err());
}

/// 【文件补丁测试】【匹配边界】重复、失效、重叠上下文和无锚点插入不选择任意位置。
/// 参数: 无；返回: 无。
#[test]
fn requires_unique_exact_non_overlapping_context() {
    assert!(update("x\nx\n", "@@\n-x\n+y").is_err());
    assert!(update(" x\n", "@@\n-x\n+y").is_err());
    assert!(update("x\n", "@@\n+new").is_err());
    assert!(update("a\nb\n", "@@\n-a\n+A\n b\n@@\n-b\n+B").is_err());
    assert_eq!(
        update("x\nx\n", "@@\n-x\n+y\n*** End of File").unwrap(),
        "x\ny\n"
    );
    assert_eq!(
        update("one\nx\ntwo\nx\n", "@@ two\n-x\n+y").unwrap(),
        "one\nx\ntwo\ny\n"
    );
    assert_eq!(
        update("a\nb\nc\n", "@@\n-a\n+A\n@@\n-c\n+C").unwrap(),
        "A\nb\nC\n"
    );
}

/// 【文件补丁测试】【新增空文件】允许零行文件和明确无末尾换行，拒绝控制字符路径。
/// 参数: 无；返回: 无。
#[test]
fn supports_empty_additions_and_validates_paths() {
    assert_eq!(
        parse("*** Begin Patch\r\n*** Add File: 空 白.txt\r\n*** End Patch\r\n").unwrap(),
        vec![Operation::Add {
            path: "空 白.txt".into(),
            content: String::new()
        }]
    );
    let operations = parse(
        "*** Begin Patch\n*** Add File: a\n+text\n\\ No newline at end of file\n*** End Patch",
    )
    .unwrap();
    assert_eq!(
        operations,
        vec![Operation::Add {
            path: "a".into(),
            content: "text".into()
        }]
    );
    assert!(parse("*** Begin Patch\n*** Add File: \ta\n+x\n*** End Patch").is_err());
}

/// 【文件补丁测试】【资源上限】超长补丁和过多文件在任何文件操作前直接拒绝。
/// 参数: 无；返回: 无。
#[test]
fn rejects_oversized_patches_and_excessive_file_counts() {
    assert!(
        parse(&"x".repeat(super::model::MAX_PATCH_BYTES + 1))
            .unwrap_err()
            .to_string()
            .contains("limit")
    );
    let files = (0..65)
        .map(|index| format!("*** Add File: file-{index}\n+new\n"))
        .collect::<String>();
    assert!(
        parse(&format!("*** Begin Patch\n{files}*** End Patch"))
            .unwrap_err()
            .to_string()
            .contains("64-file")
    );
}
