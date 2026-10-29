//! 真实时间线入口的默认预览、显式全文展开与缓存回归。

use super::super::{cell::HistoryCell, test_support::options, tool_cell::ToolCell};
use super::*;
use crate::render::{activity_animation::strip_ansi_for_test as plain, tool_view::ToolView};
use serde_json::{json, Value};

/// 【终端】【预览样本】把已完成的工具放入时间线，避免真实工具副作用。
/// 参数: 名称、参数与结果；返回: 含单一工具的记录。
fn store(name: &str, args: Value, output: &str) -> TranscriptStore {
    let mut view = ToolView::running(name.into(), args.to_string());
    view.finish(true, output.into());
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::Tool(ToolCell::Invocation(Box::new(view))));
    store
}

/// 【终端】【预览文本】通过完整时间线重排生成正文，展开集合模拟用户点击。
/// 参数: 记录及展开键；返回: 纯文本结果。
fn text(store: &mut TranscriptStore, expanded: &HashSet<ParagraphKey>) -> String {
    store
        .render_fullscreen(100, &options(), expanded)
        .lines
        .iter()
        .map(|line| plain(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【终端】【链接预览回归】默认详情保留标题和域名，点击整块后展示完整地址且缓存不混用。
/// 参数: 无；返回: 无。
#[test]
fn recent_web_preview_can_expand_full_url_and_restore_preview() {
    let mut store = store("web_search", json!({"query":"docs"}), "**Provider**: Fixture\n### 1. Guide\n**URL**: https://example.com/guide?tracking=full-value\n**Snippet**: Read the documentation.");
    let default = store.render_fullscreen(100, &options(), &HashSet::new());
    let key = default
        .paragraphs
        .iter()
        .find(|span| span.key.cell == 0)
        .unwrap()
        .key;
    assert_eq!(key, ParagraphKey::whole(0));
    assert!(!default.paragraphs[0].expanded);
    let preview = text(&mut store, &HashSet::new());
    assert!(
        preview.contains("example.com") && !preview.contains("tracking="),
        "{preview}"
    );
    let full = text(&mut store, &HashSet::from([key]));
    assert!(
        full.contains("https://example.com/guide?tracking=full-value"),
        "{full}"
    );
    assert!(!text(&mut store, &HashSet::new()).contains("tracking="));
}

/// 【终端】【后台预览回归】默认详情显示短标识与任务状态，全文展开恢复可复制标识。
/// 参数: 无；返回: 无。
#[test]
fn recent_background_list_uses_short_ids_until_expanded() {
    let id = "background-task-long-identifier";
    let mut store = store(
        "background_command",
        json!({"action":"list"}),
        &json!({"ok":true,"tasks":[
            {"id":id,"status":"running","command":"serve"}
        ]})
        .to_string(),
    );
    let preview = text(&mut store, &HashSet::new());
    assert!(
        preview.contains("1 running") && preview.contains("backgrou…"),
        "{preview}"
    );
    assert!(!preview.contains(id));
    assert!(text(&mut store, &HashSet::from([ParagraphKey::whole(0)])).contains(id));
}

/// 【终端】【多图预览回归】真实 Kitty 协议默认只生成一张有界缩略图，展开才追加其余图片。
/// 参数: 无；返回: 无。
#[test]
fn recent_generated_images_show_one_bounded_preview() {
    use crate::render::terminal_image::test_override;
    test_override::set(Some(true), Some(false), Some(false));
    let directory = tempfile::tempdir().unwrap();
    let paths = [
        directory.path().join("first.png"),
        directory.path().join("second.png"),
    ];
    for path in &paths {
        image::RgbaImage::from_pixel(640, 480, image::Rgba([23, 117, 211, 255]))
            .save(path)
            .unwrap();
    }
    let mut store = store(
        "generate_image",
        json!({}),
        &json!({"type":"image_generation","images":[
            {"local_path":paths[0]}, {"local_path":paths[1]}
        ]})
        .to_string(),
    );
    let default = store.render_fullscreen(100, &options(), &HashSet::new());
    let protocols = default
        .lines
        .iter()
        .filter(|line| line.as_str().contains("a=p"))
        .collect::<Vec<_>>();
    assert_eq!(protocols.len(), 1);
    for key in [",c=", ",r="] {
        let size = protocols[0]
            .as_str()
            .split(key)
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .split('\x1b')
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(size <= if key == ",c=" { 24 } else { 8 });
    }
    let preview = text(&mut store, &HashSet::new());
    assert!(
        preview.contains("2 images saved")
            && preview.contains("first.png")
            && preview.contains("second.png")
    );
    let expanded =
        store.render_fullscreen(100, &options(), &HashSet::from([ParagraphKey::whole(0)]));
    assert_eq!(
        expanded
            .lines
            .iter()
            .filter(|line| line.as_str().contains("a=p"))
            .count(),
        2
    );
    test_override::set(None, None, None);
}

/// 【终端】【分组预览回归】跨文件首尾折叠仍保留每段归属，展开恢复被省略的命中。
/// 参数: 无；返回: 无。
#[test]
fn grep_preview_keeps_tail_file_header() {
    let hits = ["a.rs", "b.rs"]
        .iter()
        .flat_map(|path| (1..=10).map(move |n| format!("{path}:{n}:token-{n}")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut store = store(
        "grep",
        json!({"pattern":"token"}),
        &json!({"success":true,"stdout":hits}).to_string(),
    );
    let preview = text(&mut store, &HashSet::new());
    assert!(
        preview.contains("a.rs") && preview.contains("b.rs"),
        "{preview}"
    );
    assert!(
        preview.find("b.rs").unwrap() < preview.find("10: token-10").unwrap(),
        "{preview}"
    );
    assert!(!preview.contains("5: token-5"));
    assert!(text(&mut store, &HashSet::from([ParagraphKey::whole(0)])).contains("5: token-5"));
}

/// 【终端】【通用审批回归】真实非标准节点显示目标及审阅范围，不展示任意正文参数。
/// 参数: 无；返回: 无。
#[test]
fn generic_timeline_approval_uses_safe_context() {
    let mut view = ToolView::running(
        "custom_write".into(),
        json!({"path":"/tmp/a\rb.txt", "content":"PRIVATE_BODY"}).to_string(),
    );
    view.request_permission("request".into());
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::Tool(ToolCell::Invocation(Box::new(view))));
    let preview = text(&mut store, &HashSet::new());
    assert!(
        preview.contains("/tmp/a\\rb.txt") && preview.contains("Review scope:"),
        "{preview}"
    );
    assert!(!preview.contains('\r') && !preview.contains("PRIVATE_BODY"));
    assert!(preview.find("/tmp/").unwrap() < preview.find("Allow once").unwrap());
}

/// 【终端】【待办预览回归】默认历史只保留变更，完整计划必须显式展开且保持原序。
/// 参数: 无；返回: 无。
#[test]
fn recent_todo_keeps_change_record_until_plan_is_expanded() {
    let mut store = store("todo", json!({"action":"update","status":"completed"}), &json!({
        "changed":[{"id":"one","text":"first task","status":"completed"}],
        "items":[{"id":"one","text":"first task","status":"completed"},{"id":"two","text":"second task","status":"pending"}]
    }).to_string());
    let preview = text(&mut store, &HashSet::new());
    assert!(
        preview.contains("first task") && !preview.contains("Plan:"),
        "{preview}"
    );
    let full = text(&mut store, &HashSet::from([ParagraphKey::whole(0)]));
    assert!(
        full.contains("Plan:")
            && full.find("first task").unwrap() < full.find("second task").unwrap()
    );
    assert_eq!(full.matches("first task").count(), 1, "{full}");
}
