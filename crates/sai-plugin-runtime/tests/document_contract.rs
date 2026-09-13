#[path = "document/support.rs"]
mod support;

use serde_json::json;

/// 【文档测试】【字符摘录】UTF-8 替换、总字符数和按字符裁剪必须同时准确
/// @returns 无；无效字节不按 charset 解码，字符不能截成半个字节序列
#[tokio::test]
async fn documents_preserve_lossy_utf8_and_count_all_characters() {
    let bytes = b"a\xe4\xb8\xad\xff\xf0\x9f\x98\x80z".to_vec();
    let plugin = support::plugin(
        &format!("{} return data:document(args)", support::INPUT),
        bytes,
        json!({}),
    );
    for (limit, text, truncated) in [
        (1, "a", true),
        (3, "a中\u{fffd}", true),
        (5, "a中\u{fffd}\u{1f600}z", false),
        (8, "a中\u{fffd}\u{1f600}z", false),
    ] {
        assert_eq!(
            support::run(&plugin, json!({"max_chars":limit}))
                .await
                .unwrap(),
            json!({"text":text,"total_chars":5,"truncated":truncated})
        );
    }
}

/// 【文档测试】【代表性兼容】普通 HTML 样本与原 Markdown 和纯文本转换器结果对照
/// @returns 无；转换后的总字符数在裁剪之前计算，宽度参数影响实际换行
#[tokio::test]
async fn html_modes_match_the_existing_libraries_before_clipping() {
    let html = "<h2>标题 &amp; topic</h2><p>one <strong>two</strong> <a href='/a'>link</a></p><pre>a\nb</pre>";
    let plugin = support::plugin(
        &format!("{} return data:document(args)", support::INPUT),
        html.as_bytes().to_vec(),
        json!({}),
    );
    for (mode, expected) in [
        ("raw", html.to_string()),
        ("html_text", html2text::from_read(html.as_bytes(), 120)),
        ("html_markdown", html2md::parse_html(html)),
    ] {
        for limit in [4, 24000] {
            assert_eq!(
                support::run(&plugin, json!({"mode":mode,"max_chars":limit}))
                    .await
                    .unwrap(),
                json!({
                    "text":expected.chars().take(limit).collect::<String>(),
                    "total_chars":expected.chars().count(),"truncated":expected.chars().count()>limit
                })
            );
        }
    }
    let expected = html2text::from_read(html.as_bytes(), 20);
    assert_eq!(
        support::run(&plugin, json!({"mode":"html_text","width":20}))
            .await
            .unwrap()["text"],
        expected
    );
}

/// 【文档测试】【选项边界】不接受未知转换、无效数量、错误类型和隐式参数
/// @returns 无；失败不会破坏输入句柄，后续合法读取仍然成功
#[tokio::test]
async fn invalid_document_options_fail_without_consuming_the_buffer() {
    let plugin = support::plugin(
        r#"
        local data=sai.binary.from_bytes('abc')
        local ok=pcall(data.document,data,args.options)
        assert(not ok)
        return data:document({max_chars=2})
    "#,
        vec![],
        json!({}),
    );
    for options in [
        json!({"mode":"pdf"}),
        json!({"max_chars":0}),
        json!({"max_chars":-1}),
        json!({"max_chars":1.5}),
        json!({"max_chars":"1"}),
        json!({"width":0}),
        json!({"width":201}),
        json!({"timeout_ms":0}),
        json!({"extra":true}),
        json!(false),
    ] {
        assert_eq!(
            support::run(&plugin, json!({"options":options}))
                .await
                .unwrap(),
            json!({"text":"ab","total_chars":3,"truncated":true})
        );
    }
}
