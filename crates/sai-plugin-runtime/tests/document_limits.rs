#[path = "document/support.rs"]
mod support;

use serde_json::json;

/// 【文档测试】【完整大正文】五 MiB 原始输入不经过四 MiB 的普通文本接口
/// @returns 无；普通及全部无效 UTF-8 输入都保留准确字符总数
#[tokio::test]
async fn five_mib_documents_fit_the_shared_execution_and_binary_limits() {
    for byte in [b'x', 0xff] {
        let plugin = support::plugin(
            &format!("{} return data:document(args)", support::INPUT),
            vec![byte; 5 * 1024 * 1024],
            json!({
                "instructions":20_000_000,"binary_bytes":64*1024*1024,"timeout_ms":30000
            }),
        );
        for mode in ["raw", "html_text", "html_markdown"] {
            let result = support::run(&plugin, json!({"mode":mode,"max_chars":11}))
                .await
                .unwrap();
            assert_eq!(
                result["text"],
                if byte == b'x' {
                    "x".repeat(11)
                } else {
                    "\u{fffd}".repeat(11)
                }
            );
            assert!(result["total_chars"].as_u64().unwrap() >= 5 * 1024 * 1024);
            assert_eq!(result["truncated"], true);
        }
    }
}

/// 【文档测试】【完整输出边界】输出按整个结构化结果计量，包含控制字符的 JSON 转义
/// @returns 无；超限返回错误，较小摘录及原始缓冲继续可用
#[tokio::test]
async fn document_json_output_is_bounded_without_silent_clipping() {
    let plugin = support::plugin(
        r#"
        local data=sai.binary.from_bytes(string.rep(string.char(0),300))
        local ok,message=pcall(data.document,data,{max_chars=300})
        assert(not ok and tostring(message):find('output limit',1,true),tostring(message))
        return data:document({max_chars=100})
    "#,
        vec![],
        json!({"output_bytes":1024}),
    );
    assert_eq!(
        support::run(&plugin, json!({})).await.unwrap(),
        json!({"text":"\0".repeat(100),"total_chars":300,"truncated":true})
    );
}

/// 【文档测试】【共用计算】原生转换与 Lua 共享可信指令额度，公开字段不能扩大限制
/// @returns 无；低额度失败，高额度执行同样内容可完成
#[tokio::test]
async fn document_processing_uses_the_trusted_instruction_budget() {
    let body = format!(
        "sai.limits.instructions=1000000000 {} return data:document({{max_chars=1}})",
        support::INPUT
    );
    for (instructions, success) in [(10000, false), (100000, true)] {
        let plugin = support::plugin(
            &body,
            vec![b'a'; 20000],
            json!({"instructions":instructions}),
        );
        let result = support::run(&plugin, json!({})).await;
        assert_eq!(result.is_ok(), success, "{result:?}");
        if !success {
            assert!(format!("{:#}", result.unwrap_err()).contains("instruction budget"));
        }
    }
}

/// 【文档测试】【共同预留】已有二进制缓冲占用的额度不能再次用于转换工作空间
/// @returns 无；关闭占用缓冲后可重试，重复处理会归还本次预留
#[tokio::test]
async fn document_workspaces_share_and_release_the_binary_budget() {
    let plugin = support::plugin(
        &format!(
            r#"
        {input}
        local small=sai.binary.from_bytes('text')
        local ok,message=pcall(small.document,small,{{max_chars=2}})
        assert(not ok and tostring(message):find('size limit',1,true),tostring(message))
        data:close()
        for index=1,10 do
            assert(small:document({{max_chars=2}}).text=='te')
        end
        return true
    "#,
            input = support::INPUT
        ),
        vec![b'x'; 250 * 1024],
        json!({"binary_bytes":256*1024,"output_bytes":1024}),
    );
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), true);
}
