//! 面板交互的真实浏览器测试：设置 SAI_BROWSER_E2E=1 时运行。

use super::events::BrowserEvent;
use super::{BrowserSession, MouseInput, ProfileMode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// 【面板测试】【开关】未设置环境变量时跳过真实浏览器测试。
/// @returns 需要运行时为 true
fn enabled() -> bool {
    std::env::var("SAI_BROWSER_E2E").ok().as_deref() == Some("1")
}

/// 【面板测试】【启动】用临时用户目录启动浏览器并打开测试页面。
/// @param html 为页面 HTML
/// @returns 浏览器会话
async fn open(html: &str) -> Arc<BrowserSession> {
    let session = BrowserSession::launch_with(ProfileMode::Temporary)
        .await
        .unwrap();
    let url = format!("data:text/html;charset=utf-8,{}", urlencoding::encode(html));
    session.navigate(&url).await.unwrap();
    session
}

/// 【面板测试】【事件等待】等待满足条件的会话事件。
/// @param events 为事件订阅；matcher 为匹配函数
/// @returns 匹配到的事件
async fn wait_event<T>(
    events: &mut broadcast::Receiver<BrowserEvent>,
    mut matcher: impl FnMut(&BrowserEvent) -> Option<T>,
) -> T {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(event) = events.recv().await {
                if let Some(value) = matcher(&event) {
                    return value;
                }
            }
        }
    })
    .await
    .expect("event within timeout")
}

/// 【面板测试】【左键按下】在页面坐标处发送一次左键按下与抬起。
/// @param session 为浏览器会话；x、y 为页面坐标
/// @returns 无
async fn click_at(session: &BrowserSession, x: f64, y: f64) {
    for kind in ["down", "up"] {
        let input: MouseInput = serde_json::from_value(serde_json::json!({
            "kind": kind, "x": x, "y": y, "button": "left", "click_count": 1
        }))
        .unwrap();
        session.dispatch_mouse_input(&input).await.unwrap();
    }
}

/// 无头标识被替换，webdriver 不暴露，屏幕尺寸不小于视口。
#[tokio::test]
async fn launch_hides_automation_markers() {
    if !enabled() {
        return;
    }
    let session = open("<title>x</title>").await;
    let value = session
        .evaluate(
            "JSON.stringify([navigator.userAgent, navigator.webdriver, screen.width, innerWidth])",
        )
        .await
        .unwrap();
    let parsed: (String, bool, u32, u32) = serde_json::from_str(value.as_str().unwrap()).unwrap();
    assert!(!parsed.0.contains("Headless"), "{}", parsed.0);
    assert!(!parsed.1, "navigator.webdriver must be false");
    assert!(
        parsed.2 >= parsed.3,
        "screen {} < viewport {}",
        parsed.2,
        parsed.3
    );
    session.close().await;
}

/// 面板在线时 confirm 交给面板回复，回复结果写回页面；离线时自动处理不阻塞。
#[tokio::test]
async fn dialogs_wait_for_the_panel_and_resolve() {
    if !enabled() {
        return;
    }
    let session =
        open("<button onclick=\"document.title = confirm('sure?') ? 'yes' : 'no'\">go</button>")
            .await;
    let mut events = session.subscribe();
    session.attach_viewer().await;
    session
        .evaluate("setTimeout(() => document.querySelector('button').click(), 0); true")
        .await
        .unwrap();
    let dialog = wait_event(&mut events, |event| match event {
        BrowserEvent::Dialog(Some(info)) => Some(info.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        (dialog.kind.as_str(), dialog.message.as_str()),
        ("confirm", "sure?")
    );
    session.answer_dialog(true, None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(session.evaluate("document.title").await.unwrap(), "yes");
    // 面板离开后自动取消 confirm，页面脚本不再阻塞
    session.detach_viewer().await;
    session
        .evaluate("setTimeout(() => document.querySelector('button').click(), 0); true")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(session.evaluate("document.title").await.unwrap(), "no");
    session.close().await;
}

/// 点开原生下拉框时面板收到选项，选中后页面触发 change。
#[tokio::test]
async fn native_select_opens_in_the_panel() {
    if !enabled() {
        return;
    }
    let session = open(
        "<select id=s style='position:absolute;left:20px;top:20px;width:200px;height:30px' \
         onchange=\"document.title='picked:'+this.value\">\
         <option value=a>Alpha</option><optgroup label=More><option value=b>Beta</option></optgroup></select>",
    )
    .await;
    session.install_page_hooks().await.unwrap();
    let mut events = session.subscribe();
    click_at(&session, 60.0, 35.0).await;
    session.take_select_popup().await;
    let popup = wait_event(&mut events, |event| match event {
        BrowserEvent::SelectPopup(popup) => Some(popup.clone()),
        _ => None,
    })
    .await;
    let labels: Vec<_> = popup
        .options
        .iter()
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(labels, ["Alpha", "Beta"]);
    assert_eq!(popup.options[1].group, "More");
    session.apply_select(1).await.unwrap();
    assert_eq!(
        session.evaluate("document.title").await.unwrap(),
        "picked:b"
    );
    session.close().await;
}

/// 元素选择返回被点击元素的信息，页面自身的点击处理不会触发。
#[tokio::test]
async fn element_picker_returns_the_clicked_element() {
    if !enabled() {
        return;
    }
    let session = open(
        "<button id=target style='position:absolute;left:40px;top:40px;width:120px;height:40px' \
         onclick=\"document.title='page-click'\">Buy now</button>",
    )
    .await;
    let picker = {
        let session = session.clone();
        tokio::spawn(async move { session.pick_element(&["Background", "Color", "Font"]).await })
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    for kind in ["move", "down", "up"] {
        let input: MouseInput = serde_json::from_value(serde_json::json!({
            "kind": kind, "x": 100, "y": 60, "button": if kind == "move" { "none" } else { "left" }, "click_count": 1
        }))
        .unwrap();
        session.dispatch_mouse_input(&input).await.unwrap();
    }
    let element = picker.await.unwrap().unwrap().expect("picked element");
    assert_eq!(element["tagName"], "button");
    assert_eq!(element["selector"], "#target");
    assert_eq!(element["text"], "Buy now");
    assert_ne!(
        session.evaluate("document.title").await.unwrap(),
        "page-click"
    );
    // 取消选择返回空结果
    let picker = {
        let session = session.clone();
        tokio::spawn(async move { session.pick_element(&["Background", "Color", "Font"]).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    session.cancel_pick().await.unwrap();
    assert!(picker.await.unwrap().unwrap().is_none());
    session.close().await;
}

/// 复制快捷键把页面选中文本交给面板；文件选择请求交给面板并能写回输入框。
#[tokio::test]
async fn copy_and_file_chooser_reach_the_panel() {
    if !enabled() {
        return;
    }
    let session = open(
        "<p id=p>copy me please</p><input type=file id=f style='position:absolute;left:20px;top:80px' \
         onchange=\"document.title=this.files[0].name+':'+this.files[0].size\">",
    )
    .await;
    let mut events = session.subscribe();
    // 1. 选中段落后按 Ctrl+C
    session
        .evaluate("getSelection().selectAllChildren(document.getElementById('p')); true")
        .await
        .unwrap();
    let key: super::KeyInput = serde_json::from_value(serde_json::json!({
        "kind": "down", "key": "c", "code": "KeyC", "key_code": 67, "modifiers": 2
    }))
    .unwrap();
    session.dispatch_panel_key(&key).await.unwrap();
    let copied = wait_event(&mut events, |event| match event {
        BrowserEvent::Clipboard(text) => Some(text.clone()),
        _ => None,
    })
    .await;
    assert_eq!(copied, "copy me please");
    // 2. 点击文件输入框，面板收到文件选择请求，上传后页面拿到文件
    click_at(&session, 60.0, 90.0).await;
    wait_event(&mut events, |event| match event {
        BrowserEvent::FileChooser(Some(_)) => Some(()),
        _ => None,
    })
    .await;
    let id = super::store_upload("报告.txt", b"hello").unwrap();
    session.answer_file_chooser(&[id]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        session.evaluate("document.title").await.unwrap(),
        "报告.txt:5"
    );
    session.close().await;
    super::uploads::remove_uploads();
}

/// 页面下载落到 Sai 下载目录，完成后可按 GUID 取回原文件名与内容。
#[tokio::test]
async fn downloads_complete_and_can_be_fetched() {
    if !enabled() {
        return;
    }
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().unwrap().port();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Ok((mut stream, _)) = server.accept().await {
            let mut buffer = [0u8; 2048];
            let read = stream.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let (kind, body) = if request.starts_with("GET /file") {
                ("application/octet-stream\r\nContent-Disposition: attachment; filename=\"report.txt\"", "hello download")
            } else {
                (
                    "text/html",
                    "<title>dl</title><a id=dl href='/file'>get</a>",
                )
            };
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });
    let session = BrowserSession::launch_with(ProfileMode::Temporary)
        .await
        .unwrap();
    session
        .navigate(&format!("http://127.0.0.1:{port}/"))
        .await
        .unwrap();
    let mut events = session.subscribe();
    session
        .evaluate("document.getElementById('dl').click(); true")
        .await
        .unwrap();
    let done = wait_event(&mut events, |event| match event {
        BrowserEvent::Download(info) if info.state == "completed" => Some(info.clone()),
        _ => None,
    })
    .await;
    assert_eq!(done.file_name, "report.txt");
    let (path, name) = session
        .completed_download(&done.guid)
        .expect("completed download");
    assert_eq!(name, "report.txt");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello download");
    let _ = std::fs::remove_file(path);
    assert!(session.is_alive(), "browser must survive the download");
    session.close().await;
}
