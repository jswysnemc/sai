use super::*;

/// 验证无标签及路由关闭时逐字保留原文，无参数、无返回值。
#[test]
fn plain_and_disabled_are_byte_identical() {
    let text = "开头\n\n<jev description=\"测试\">条件文字</jev>\n结尾";
    assert_eq!(baseline(text, false).unwrap(), text);
    assert_eq!(parse("普通文字\n").unwrap().baseline, "普通文字\n");
}

/// 验证多段、中文、显式描述与正文回退，无参数、无返回值。
#[test]
fn separates_segments_in_source_order() {
    let parsed =
        parse("前<jev>中文正文</jev>中<jev description=\"测试时 > 使用\">测试规则</jev>后")
            .unwrap();
    assert_eq!(parsed.baseline, "前中后");
    assert_eq!(parsed.segments[0].description, "中文正文");
    assert_eq!(parsed.segments[1].description, "测试时 > 使用");
    assert_eq!(parsed.segments[1].content, "测试规则");
}

/// 验证合法空白、单引号和空段，无参数、无返回值。
#[test]
fn accepts_spacing_and_ignores_empty_segments() {
    let parsed = parse("<jev\n description = '  ' >正文</jev><jev> \n </jev>").unwrap();
    assert_eq!(parsed.segments.len(), 1);
    assert_eq!(parsed.segments[0].description, "正文");
    assert!(parsed.baseline.is_empty());
}

/// 验证标签边界与常用其他标签互不干扰，无参数、无返回值。
#[test]
fn similar_tag_names_are_not_routed() {
    let text = "<jev-context>正常</jev-context><jevish>正常</jevish>";
    assert_eq!(parse(text).unwrap().baseline, text);
}

/// 验证错误标签不能静默暴露或丢弃正文，无参数、无返回值。
#[test]
fn invalid_tags_fail_explicitly() {
    for text in [
        "<jev",
        "<jev>secret",
        "</jev>",
        "<jev description=no>secret</jev>",
        "<jev other=\"x\">secret</jev>",
        "<jev><jev>x</jev></jev>",
        "<jev description=\"x\" extra=\"y\">secret</jev>",
    ] {
        assert!(parse(text).is_err(), "{text}");
        assert_eq!(baseline(text, false).unwrap(), text);
    }
}
