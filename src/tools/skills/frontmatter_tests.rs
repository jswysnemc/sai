use super::{frontmatter::parse_skill_frontmatter, skill_catalog, tests::test_paths};
use crate::config::AppConfig;
use crate::tools::skill_management::{create_managed_skill, list_managed_skills};

/// 验证补全目录显示 YAML 折叠简介的正文，不显示块标量标记
///
/// 返回：无返回值，目录简介错误时断言失败
#[test]
fn skill_catalog_decodes_folded_description() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let skill_dir = paths.skills_dir.join("patch-review-brief");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: patch-review-brief\ndescription: >-\n  Review the current patch\n  against the project standards.\n---\n\n# Review instructions\n",
    )
    .unwrap();

    let entries = skill_catalog(&AppConfig::default(), &paths).unwrap();
    let entry = entries
        .iter()
        .find(|entry| entry.name == "patch-review-brief")
        .unwrap();
    assert_eq!(
        entry.description,
        "Review the current patch against the project standards."
    );
}

/// 验证 YAML 多行、引号、注释和缩进标记都转换成可阅读简介
///
/// 返回：无返回值，任一标量解析错误时断言失败
#[test]
fn decodes_yaml_description_formats() {
    let cases = [
        (">\n  First line\n  second line\n", "First line second line"),
        (
            ">+\n  First line\n\n  second paragraph\n\n",
            "First line second paragraph",
        ),
        (
            "|-\n  First line\n  second line\n",
            "First line second line",
        ),
        (
            "|+\n  First line\n  second line\n\n",
            "First line second line",
        ),
        (
            ">2- # summary\n  First line\n  second line\n",
            "First line second line",
        ),
        (
            "'Review: it''s ready # now' # comment\n",
            "Review: it's ready # now",
        ),
        (
            "\"Review \\\"quoted\\\" text\\nnext line\"\n",
            "Review \"quoted\" text next line",
        ),
        ("Review code # comment\n", "Review code"),
        (
            "Review code\n  against standards\n",
            "Review code against standards",
        ),
        ("\"First line\n  second line\"\n", "First line second line"),
        (
            "|-\n  检查代码改动\n  并整理评估结果\n",
            "检查代码改动 并整理评估结果",
        ),
    ];
    for (scalar, expected) in cases {
        let raw = format!("---\nname: review\ndescription: {scalar}---\n# Body\n");
        assert_eq!(
            parse_skill_frontmatter(&raw).description.as_deref(),
            Some(expected),
            "scalar: {scalar}"
        );
    }
}

/// 验证元数据只读取 YAML 顶层，兼容 BOM 与 CRLF，保留原始文档
///
/// 返回：无返回值，字段来源或文档边界错误时断言失败
#[test]
fn respects_top_level_fields_and_frontmatter_boundaries() {
    let raw = "\u{feff}---\r\nmetadata:\r\n  name: wrong-name\r\n  description: Wrong summary\r\nname: 'review'\r\ndescription: |-\r\n  Read the patch\r\n  ---\r\n  and review it\r\n---\r\ndescription: Body text\r\n";
    let metadata = parse_skill_frontmatter(raw);
    assert_eq!(metadata.name.as_deref(), Some("review"));
    assert_eq!(
        metadata.description.as_deref(),
        Some("Read the patch --- and review it")
    );
    assert_eq!(
        parse_skill_frontmatter("---\nname: review\ndescription: Good\n...\nBody")
            .description
            .as_deref(),
        Some("Good")
    );

    for raw in [
        "# Body\ndescription: Wrong summary",
        "---\nname: review\ndescription: Unclosed header",
        "---\nmetadata:\n  description: Nested summary\n---",
        "---\nname: review\ndescription: [invalid\n---",
        "---\nname: review\ndescription: [a, b]\n---",
        "---\nname: review\ndescription: null\n---",
        "---\nname: review\ndescription: 42\n---",
        "---\nname: review\ndescription: >-\n---",
    ] {
        assert!(
            parse_skill_frontmatter(raw).description.is_none(),
            "raw: {raw}"
        );
    }
}

/// 验证技能创建校验、设置列表与补全目录共享同一 YAML 解析行为
///
/// 返回：无返回值，创建失败或两处简介不一致时断言失败
#[test]
fn managed_skills_share_catalog_description_parsing() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let raw = "---\nname: yaml-review-fixture\ndescription: |-\n  Review changes\n  against standards\n---\n\n# Instructions\n";
    let id = create_managed_skill("yaml-review-fixture", raw, &paths).unwrap();
    let config = AppConfig::default();
    let managed = list_managed_skills(&config, &paths).unwrap();
    let entry = managed.iter().find(|entry| entry.id == id).unwrap();
    assert_eq!(entry.description, "Review changes against standards");
    let catalog = skill_catalog(&config, &paths).unwrap();
    assert_eq!(
        catalog
            .iter()
            .find(|skill| skill.name == entry.name)
            .unwrap()
            .description,
        entry.description
    );
    assert_eq!(
        std::fs::read_to_string(paths.skills_dir.join("yaml-review-fixture/SKILL.md")).unwrap(),
        raw
    );
}
