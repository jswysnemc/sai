mod common;
#[path = "common/vision.rs"]
mod vision;

use sai_plugin_runtime::{EventContext, EventKind};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use vision::*;

/// 【视觉测试】【取消释放】取消回调释放模型 Future 和图片预算，同一实例能够继续处理新调用。
/// @returns 无；旧句柄不能借用新回调权限
#[tokio::test]
async fn cancellation_releases_the_image_request_and_next_callback_recovers() {
    let plugin = runtime(
        r#"
        local old
        sai.register_tool({name='run',description='Cancel vision',parameters={type='object'},execute=function()
            if old then
                local ok,err=pcall(old.analyze_image,old,{prompt='old',mime_type='image/png'})
                assert(not ok and tostring(err):find('expired',1,true))
            end
            old=sai.binary.decode_base64(string.rep('YWFh',341)..'YQ==')
            return old:analyze_image({prompt='describe',mime_type='image/png'}).model
        end})
    "#,
        capabilities(),
        |limits| limits.binary_bytes = 1024,
    );
    let services = Arc::new(Services::default());
    services.block.store(true, Ordering::SeqCst);
    let running = plugin.clone();
    let ctx = context(services.clone());
    let task = tokio::spawn(async move { running.call_tool("run", json!({}), ctx).await });
    tokio::time::timeout(Duration::from_secs(2), services.entered.notified())
        .await
        .unwrap();
    assert_eq!(services.active.load(Ordering::SeqCst), 1);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), services.released.notified())
        .await
        .unwrap();
    assert_eq!(services.active.load(Ordering::SeqCst), 0);
    services.block.store(false, Ordering::SeqCst);
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(services.clone()))
            .await
            .unwrap(),
        "vision-model"
    );
    assert_eq!(services.requests.lock().unwrap().len(), 2);
}

/// 【视觉测试】【超时回收】单次视觉超时可由 Lua 捕获，模型 Future 随即释放。
/// @returns 无；超时后仍能读取当前回调内的图片
#[tokio::test]
async fn timeout_releases_vision_work_without_invalidating_the_active_buffer() {
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Vision timeout',parameters={type='object'},execute=function()
            local b=sai.binary.decode_base64('YWJj')
            local ok,err=pcall(b.analyze_image,b,{prompt='describe',mime_type='image/png',timeout_ms=15})
            assert(not ok and b:bytes(0,3)=='abc')
            return tostring(err)
        end})
    "#,
        capabilities(),
        |_| {},
    );
    let services = Arc::new(Services::default());
    services.block.store(true, Ordering::SeqCst);
    assert!(plugin
        .call_tool("run", json!({}), context(services.clone()))
        .await
        .unwrap()
        .contains("vision request timed out"));
    assert_eq!(services.active.load(Ordering::SeqCst), 0);
}

/// 【视觉测试】【事件隔离】加载和事件回调均不能取得工具回调绑定的模型服务。
/// @returns 无；完成回调后不保留宿主服务引用
#[tokio::test]
async fn initialization_and_events_cannot_reuse_vision_services() {
    let plugin = runtime(
        r#"
        assert(not pcall(sai.vision.info))
        sai.register_tool({name='run',description='Use vision',parameters={type='object'},execute=function()
            return sai.binary.decode_base64('YWJj'):analyze_image({prompt='describe',mime_type='image/png'}).model
        end})
        sai.on('agent_start',function()
            local info_ok,info_error=pcall(sai.vision.info)
            local b=sai.binary.decode_base64('YWJj')
            local image_ok,image_error=pcall(b.analyze_image,b,{prompt='event',mime_type='image/png'})
            assert(not info_ok and not image_ok)
            assert(tostring(image_error):find('services are unavailable',1,true))
            return tostring(info_error)
        end)
    "#,
        capabilities(),
        |_| {},
    );
    let services = Arc::new(Services::default());
    let weak = Arc::downgrade(&services);
    plugin
        .call_tool("run", json!({}), context(services.clone()))
        .await
        .unwrap();
    drop(services);
    assert!(weak.upgrade().is_none());
    let result = plugin
        .emit(EventKind::AgentStart, EventContext::default())
        .await
        .unwrap();
    assert!(result[0]
        .as_str()
        .unwrap()
        .contains("services are unavailable"));
}
