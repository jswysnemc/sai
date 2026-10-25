use super::frame::ScreenshotFrame;
use super::test_support::*;
use base64::Engine;
use serde_json::json;

/// 【桌面测试】【帧投影】裁剪与缩放同时存在时，以实际交付尺寸而非请求比例投影。
/// 参数: 无；返回: 无。
#[test]
fn crop_and_rounded_scale_project_to_original_window_pixels() {
    let frame = ScreenshotFrame::new(
        (100, 60),
        &json!({"region":{"x":20,"y":10,"width":61,"height":31},"scale":0.5}),
    )
    .unwrap();
    assert_eq!(frame.image, (31, 16));
    assert_eq!(frame.project(0.0, 0.0).unwrap(), (20.0, 10.0));
    assert_eq!(frame.project(15.5, 8.0).unwrap(), (50.5, 25.5));
    for (x, y) in [
        (31.0, 0.0),
        (0.0, 16.0),
        (-1.0, 0.0),
        (f64::NAN, 0.0),
        (f64::INFINITY, 0.0),
    ] {
        assert!(frame.project(x, y).is_err());
    }
}

/// 【桌面测试】【区域边界】非法裁剪或缩放不会默默裁切到另一块屏幕。
/// 参数: 无；返回: 无。
#[test]
fn invalid_regions_and_scale_are_rejected() {
    for args in [
        json!({"region":{"x":99,"y":0,"width":2,"height":1}}),
        json!({"region":{"x":0,"y":59,"width":1,"height":2}}),
        json!({"region":{"x":0,"y":0,"width":0,"height":1}}),
        json!({"region":{"x":-1,"y":0,"width":1,"height":1}}),
        json!({"scale":0.0}),
        json!({"scale":1.1}),
    ] {
        assert!(ScreenshotFrame::new((100, 60), &args).is_err(), "{args}");
    }
}

/// 【桌面测试】【默认图像】默认帧不二次编码，启用变换时交付图片尺寸与坐标契约一致。
/// 参数: 无；返回: 无。
#[test]
fn default_frame_preserves_bytes_and_transformed_frame_matches_dimensions() {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(screenshot_base64())
        .unwrap();
    assert!(ScreenshotFrame::new((100, 60), &json!({}))
        .unwrap()
        .transform(&bytes)
        .unwrap()
        .is_none());
    let frame = ScreenshotFrame::new((100, 60), &json!({"scale":0.5})).unwrap();
    let transformed = frame.transform(&bytes).unwrap().unwrap();
    let image = image::load_from_memory(&transformed).unwrap();
    assert_eq!((image.width(), image.height()), frame.image);
}

/// 【桌面测试】【图像闭环】经过真实会话交付裁剪图，点击依然发送原图坐标及原生捕获证明。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn cropped_capture_and_input_share_one_frame() {
    let (mut session, driver) = session(vec![
        state(true),
        json!({"structuredContent":{"effect":"clicked"}}),
    ]);
    let mut args = observe_args();
    args["region"] = json!({"x":20,"y":10,"width":60,"height":40});
    args["scale"] = json!(0.5);
    let observed = session.execute(args).await.unwrap();
    assert!(observed.content.contains("Screenshot: 30x20"));
    assert!(observed.content.contains("frame_id:"));
    let bytes = observed.model_attachments[0]
        .image_url
        .strip_prefix("data:image/png;base64,")
        .unwrap();
    let image = image::load_from_memory(
        &base64::engine::general_purpose::STANDARD
            .decode(bytes)
            .unwrap(),
    )
    .unwrap();
    assert_eq!((image.width(), image.height()), (30, 20));
    let mut click = input_args(&observed.content, "click");
    click["x"] = json!(15);
    click["y"] = json!(10);
    session.execute(click.clone()).await.unwrap();
    assert!(session.execute(click).await.is_err());
    let calls = driver.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].1["x"], 50.0);
    assert_eq!(calls[1].1["y"], 30.0);
    assert_eq!(calls[1].1["capture_id"], "capture-fixture");
    assert_eq!(calls[1].1["coordinate_frame"], "window");
    assert_eq!(calls[1].1["delivery_mode"], "background");
}

/// 【桌面测试】【输入一致性】拖拽两端、滚动目标及键盘文本目标均使用同一图像投影。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn all_pixel_input_variants_project_before_dispatch() {
    for (action, extra) in [
        ("drag", json!({"from_x":1,"from_y":2,"to_x":10,"to_y":12})),
        ("scroll", json!({"x":10,"y":12,"direction":"down"})),
        ("press_key", json!({"x":10,"y":12,"key":"Return"})),
        ("type_text", json!({"x":10,"y":12,"text":"hello"})),
    ] {
        let (mut session, driver) = session(vec![
            state(true),
            json!({"structuredContent":{"effect":"applied"}}),
        ]);
        let mut args = observe_args();
        args["scale"] = json!(0.5);
        let observed = session.execute(args).await.unwrap();
        let mut input = input_args(&observed.content, action);
        input
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        session.execute(input).await.unwrap();
        let calls = driver.calls.lock().unwrap();
        let payload = &calls[1].1;
        if action == "drag" {
            assert_eq!(payload["from_x"], 2.0);
            assert_eq!(payload["from_y"], 4.0);
            assert_eq!(payload["to_x"], 20.0);
            assert_eq!(payload["to_y"], 24.0);
        } else {
            assert_eq!(payload["x"], 20.0);
            assert_eq!(payload["y"], 24.0);
        }
    }
}

/// 【桌面测试】【观察选项】驱动尺寸选项正确传递，缺少图片时不能伪造裁剪或缩放帧。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn screenshot_options_are_forwarded_and_require_an_actual_image() {
    // 1. 【桌面测试】【尺寸转发】窗口原图尺寸由驱动控制，交付缩放不会改变该请求
    let (mut observed_session, driver) = session(vec![state(true)]);
    let mut args = observe_args();
    args["max_image_dimension"] = json!(128);
    args["scale"] = json!(0.5);
    let observed = observed_session.execute(args).await.unwrap();
    assert!(observed.content.contains("Screenshot: 50x30"));
    assert_eq!(
        driver.calls.lock().unwrap()[0].1["max_image_dimension"],
        128
    );
    // 2. 【桌面测试】【缺图拒绝】不回退到原图坐标或没有图像证明的像素输入
    for options in [
        json!({"scale":0.5}),
        json!({"region":{"x":0,"y":0,"width":10,"height":10}}),
    ] {
        let (mut missing_session, driver) = session(vec![state(false)]);
        let mut args = observe_args();
        args.as_object_mut()
            .unwrap()
            .extend(options.as_object().unwrap().clone());
        let error = missing_session.execute(args).await.err().unwrap();
        assert!(error.to_string().contains("screenshot_unavailable"));
        assert_eq!(driver.calls.lock().unwrap().len(), 1);
    }
    // 3. 【桌面测试】【纯参数拒绝】显式关闭截图后，缩放参数在访问驱动前失败
    let (mut ui_session, driver) = session(vec![]);
    let mut args = observe_args();
    args["include_screenshot"] = json!(false);
    args["scale"] = json!(0.5);
    assert!(ui_session.execute(args).await.is_err());
    assert!(driver.calls.lock().unwrap().is_empty());
}
