#[path = "document/support.rs"]
mod support;

use serde_json::json;

/// 【文档生命周期测试】【句柄失效】关闭和跨回调保留的正文不能继续转换
/// @returns 无，同一实例后续取得的新缓冲仍然可以转换
#[tokio::test]
async fn document_conversion_rejects_closed_and_previous_callback_buffers() {
    let plugin = support::plugin(
        r#"
        if args.save then
            saved = sai.binary.from_bytes('first')
            return saved:document({max_chars=2})
        end
        assert(not pcall(saved.document, saved, {}))
        local closed = sai.binary.from_bytes('closed')
        closed:close()
        assert(not pcall(closed.document, closed, {}))
        return sai.binary.from_bytes('current'):document({max_chars=2})
    "#,
        vec![],
        json!({}),
    );
    assert_eq!(
        support::run(&plugin, json!({"save":true})).await.unwrap()["text"],
        "fi"
    );
    assert_eq!(
        support::run(&plugin, json!({})).await.unwrap()["text"],
        "cu"
    );
}

/// 【文档生命周期测试】【调用计费】正文转换与其他二进制操作共用系统调用次数
/// @returns 无，第二次转换不能绕过已耗尽的调用预算
#[tokio::test]
async fn document_conversion_consumes_the_system_call_budget() {
    let plugin = support::plugin(
        r#"
        local data = sai.binary.from_bytes('sample')
        assert(data:document({max_chars=1}).text == 's')
        local ok, message = pcall(data.document, data, {max_chars=1})
        assert(not ok and tostring(message):find('system call budget',1,true), tostring(message))
        return true
    "#,
        vec![],
        json!({"system_calls":2}),
    );
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), true);
}

/// 【文档生命周期测试】【输入硬上限】原始响应大于八 MiB 时不进入原生转换
/// @returns 无，调用者可捕获错误并关闭仍然有效的输入缓冲
#[tokio::test]
async fn oversized_document_input_is_rejected_before_conversion() {
    let plugin = support::plugin(
        &format!(
            r#"
        {}
        local ok, message = pcall(data.document, data, {{max_chars=1}})
        assert(not ok and tostring(message):find('document input exceeds',1,true), tostring(message))
        data:close()
        return true
    "#,
            support::INPUT
        ),
        vec![b'x'; 8 * 1024 * 1024 + 1],
        json!({"binary_bytes":32*1024*1024}),
    );
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), true);
}
