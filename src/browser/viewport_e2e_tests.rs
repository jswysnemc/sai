//! 高分屏画面与截图坐标的真实浏览器回归测试。

use super::{BrowserEvent, BrowserSession, MouseInput, ProfileMode};
use image::GenericImageView;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// 【视口测试】【测试页面】创建临时浏览器，使用固定尺寸色块检测截图位置。
/// @returns 浏览器会话；未开启真实浏览器测试时为空
async fn open() -> Option<Arc<BrowserSession>> {
    if std::env::var("SAI_BROWSER_E2E").ok().as_deref() != Some("1") {
        return None;
    }
    let session = BrowserSession::launch_with(ProfileMode::Temporary)
        .await
        .unwrap();
    session.navigate(&format!(
        "data:text/html;charset=utf-8,{}",
        urlencoding::encode(
            "<style>html{scrollbar-width:none}body{margin:0}section{height:480px;background:#f00}section+section{background:#0f0}</style>\
             <section><button style='position:absolute;left:40px;top:60px;width:100px;height:40px' \
             onclick=\"document.title='clicked'\">Target</button></section><section></section><section></section>"
        )
    )).await.unwrap();
    Some(session)
}

/// 【视口测试】【帧尺寸】解码图像并读取实际像素尺寸。
/// @param bytes 为 JPEG 字节
/// @returns 图像宽高
fn dimensions(bytes: &[u8]) -> (u32, u32) {
    image::load_from_memory(bytes).unwrap().dimensions()
}

/// 【视口测试】【等待画面】等待目标分辨率的画面帧。
/// @param events 为浏览器事件接收端；expected 为预期设备像素尺寸
/// @returns 无
async fn wait_frame(events: &mut broadcast::Receiver<BrowserEvent>, expected: (u32, u32)) {
    let mut sizes = Vec::new();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(BrowserEvent::Frame(bytes)) = events.recv().await {
                let size = dimensions(&bytes);
                sizes.push(size);
                if size == expected {
                    return;
                }
            }
        }
    })
    .await;
    assert!(result.is_ok(), "expected {expected:?}, received {sizes:?}");
}

/// 【视口测试】【像素比限制】非法输入回退为单倍，限制过大的像素比并消除微小抖动。
/// @returns 无
#[test]
fn device_scale_is_bounded_and_stable() {
    for scale in [f64::NAN, f64::INFINITY, -1.0, 0.0, 0.5] {
        assert_eq!(super::session::clamp_scale(scale), 1.0);
    }
    assert_eq!(super::session::clamp_scale(1.2500001), 1.25);
    assert_eq!(super::session::clamp_scale(3.0), 2.0);
}

/// 【视口测试】【高清推帧】录屏与新面板补发帧均使用设备像素，像素比变化后重设分辨率。
/// @returns 无
#[tokio::test]
async fn screencast_and_reconnect_frames_use_device_pixels() {
    let Some(session) = open().await else { return };
    session.resize(640, 480, 2.0).await.unwrap();
    let mut events = session.subscribe();
    session.attach_viewer().await;
    wait_frame(&mut events, (1280, 960)).await;

    // 1. 新面板的补发截图不能把已经清晰的静态画面覆盖为低分辨率
    let mut reconnect = session.subscribe();
    session.attach_viewer().await;
    let mut frames = Vec::new();
    while let Ok(event) = reconnect.try_recv() {
        if let BrowserEvent::Frame(bytes) = event {
            frames.push(dimensions(&bytes));
        }
    }
    assert!(!frames.is_empty(), "new viewers receive an initial frame");
    assert_eq!(frames.last(), Some(&(1280, 960)), "{frames:?}");

    // 2. 页面发生重绘时，持续录屏同样需要使用设备像素
    let mut live = session.subscribe();
    session
        .evaluate("document.querySelector('section').style.background='blue'")
        .await
        .unwrap();
    wait_frame(&mut live, (1280, 960)).await;

    // 3. 工具截图不能让静态页面最终停留在低分辨率的录屏帧
    let mut captured = session.subscribe();
    session.screenshot(false, None).await.unwrap();
    let mut latest = (1280, 960);
    while let Ok(Ok(event)) =
        tokio::time::timeout(Duration::from_millis(250), captured.recv()).await
    {
        if let BrowserEvent::Frame(bytes) = event {
            latest = dimensions(&bytes);
        }
    }
    assert_eq!(
        latest,
        (1280, 960),
        "tool screenshots preserve panel resolution"
    );

    // 4. CSS 尺寸不变时，改变像素比仍然重新推送对应分辨率
    for (scale, expected) in [(1.5, (960, 720)), (1.0, (640, 480))] {
        session.resize(640, 480, scale).await.unwrap();
        wait_frame(&mut events, expected).await;
        assert_eq!(session.evaluate("devicePixelRatio").await.unwrap(), scale);
    }
    session.close().await;
}

/// 【视口测试】【截图坐标】高分屏截图保持 CSS 像素尺寸，滚动后截取当前视口，点击不偏移。
/// @returns 无
#[tokio::test]
async fn screenshots_and_input_keep_css_coordinates() {
    let Some(session) = open().await else { return };
    session.resize(640, 480, 2.0).await.unwrap();
    let (jpeg, _) = session.screenshot(false, None).await.unwrap();
    assert_eq!(dimensions(&jpeg), (640, 480));
    let (full, _) = session.screenshot(true, None).await.unwrap();
    assert_eq!(dimensions(&full), (640, 1440));
    let snapshot = session.snapshot(10_000, true).await.unwrap();
    let line = snapshot
        .text
        .lines()
        .find(|line| line.contains("button \"Target\""))
        .unwrap();
    let reference = line
        .split("[ref=")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap();
    let (element, _) = session.screenshot(false, Some(reference)).await.unwrap();
    assert_eq!(dimensions(&element), (100, 40));

    // 1. 鼠标输入继续使用 CSS 坐标，不乘以屏幕像素比
    for kind in ["down", "up"] {
        let input: MouseInput = serde_json::from_value(json!({
            "kind": kind, "x": 90, "y": 80, "button": "left", "click_count": 1
        }))
        .unwrap();
        session.dispatch_mouse_input(&input).await.unwrap();
    }
    assert_eq!(session.evaluate("document.title").await.unwrap(), "clicked");

    // 2. 视口截图需要跟随滚动位置，不能因为添加裁剪参数而回到页面顶部
    session.evaluate("scrollTo(0, 480); new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))").await.unwrap();
    let (scrolled, _) = session.screenshot(false, None).await.unwrap();
    let image = image::load_from_memory(&scrolled).unwrap();
    assert_eq!(image.dimensions(), (640, 480));
    let pixel = image.get_pixel(200, 200);
    assert!(
        pixel[1] > 240 && pixel[0] < 15,
        "scrolled viewport is green: {pixel:?}"
    );
    // 3. 元素定位会再次滚动页面，裁剪原点必须使用定位后的文档偏移
    let (element, _) = session.screenshot(false, Some(reference)).await.unwrap();
    let element = image::load_from_memory(&element).unwrap();
    assert_eq!(element.dimensions(), (100, 40));
    let button = element.get_pixel(50, 5);
    assert!(
        button[0] > 200 && button[0].abs_diff(button[1]) < 10,
        "button background: {button:?}"
    );
    session.close().await;
}
