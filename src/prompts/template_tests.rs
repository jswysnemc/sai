use super::template::{render_template, validate_template};

/// 验证模板变量按名称替换，变量值中的花括号不会再次解析。
#[test]
fn renders_named_variables_once() {
    let rendered = render_template(
        "status={{ status }}\ndiff={{diff}}",
        &[
            ("status", "M src/main.rs"),
            ("diff", "const x = '{{raw}}';"),
        ],
    )
    .unwrap();

    assert_eq!(rendered, "status=M src/main.rs\ndiff=const x = '{{raw}}';");
}

/// 验证未知变量在保存前即可被拒绝。
#[test]
fn rejects_unknown_variables() {
    let error = validate_template("{{unknown}}", &["status"], &["status"]).unwrap_err();

    assert!(error.to_string().contains("unknown"));
}

/// 验证缺少必要变量时返回明确错误。
#[test]
fn rejects_missing_required_variables() {
    let error = validate_template(
        "status={{status}}",
        &["status", "diff"],
        &["status", "diff"],
    )
    .unwrap_err();

    assert!(error.to_string().contains("diff"));
}

/// 验证未闭合变量不会静默保留到运行阶段。
#[test]
fn rejects_unclosed_variable() {
    let error = validate_template("status={{status", &["status"], &["status"]).unwrap_err();

    assert!(error.to_string().contains("unclosed"));
}
