use super::knowledge_support::*;
use serde_json::{json, Value};

const PURE_ENTRY: &str = r#"
--- 【知识库对照】【纯规则入口】输入冻结样本；返回实际发布模块的计算结果
local function pure(args)
    if args.action == 'tokens' then return require('search.tokens').query(args.text) end
    if args.action == 'name' then
        local score,reason=require('search.keyword').name_score(args.text,args.name)
        return {score=score,reason=reason}
    end
    if args.action == 'chunks' then return require('embedding.chunks').build(args.text,args.size,args.overlap) end
    if args.action == 'path' then return require('paths').relative(args.text) end
    if args.action == 'cosine' then return require('search.semantic').cosine(args.left,args.right) end
    error('unknown reference action')
end
sai.register_tool({name='reference',description='Frozen knowledge rules',parameters={type='object'},execute=pure})
"#;

/// 【知识库对照】【时间归一】仅替换上传文件的时钟文字，不改变业务正文
/// @param value 实际工具值或文件快照
/// @returns 规范化后的值
fn normalize(value: Value) -> Value {
    match value {
        Value::String(text) => {
            let pattern =
                regex::Regex::new(r"(> 上传时间：)\d{4}-\d\d-\d\d \d\d:\d\d:\d\d").unwrap();
            Value::String(pattern.replace_all(&text, "${1}$$time").into_owned())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        Value::Object(items) => Value::Object(
            items
                .into_iter()
                .map(|(key, value)| (key, normalize(value)))
                .collect(),
        ),
        value => value,
    }
}

/// 【知识库对照】【业务比较】逐项比较原版结果，异常必须保留相同业务原因
/// @param actual 实际返回值；expected 为冻结结果；label 为定位信息
/// @returns 无；任何差异均使测试失败
fn assert_result(actual: anyhow::Result<Value>, expected: &Value, label: &str) {
    if expected["ok"] == true {
        let actual = normalize(actual.unwrap_or_else(|error| panic!("{label}: {error:#}")));
        assert_eq!(actual, expected["value"], "{label}");
    } else {
        let error = actual.expect_err(label);
        assert!(
            format!("{error:#}").contains(expected["error"].as_str().unwrap()),
            "{label}: {error:#}; expected {expected}"
        );
    }
}

/// 【知识库对照】【纯规则样本】固定包源码只追加测试入口，不替换任何业务函数
/// @returns 无；229 组规则、字节位置和 f32 数值样本保持一致
#[tokio::test]
async fn knowledge_matches_native_pure_rules() {
    let root = tempfile::tempdir().unwrap();
    let host = super::knowledge_host::KnowledgeHost::new(root.path());
    let runtime = configured(root.path(), &json!({}), json!({}), host, PURE_ENTRY, |_| {});
    for document in [
        include_str!("fixtures/knowledge_pure_01.json"),
        include_str!("fixtures/knowledge_pure_02.json"),
    ] {
        let fixture: Value = serde_json::from_str(document).unwrap();
        assert_eq!(
            fixture["base_commit"],
            "1a93197597af34d3702670cb79d85a2cd2406679"
        );
        for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
            assert_result(
                call(&runtime, root.path(), "reference", case.clone(), false).await,
                &case["expected"],
                &format!("pure {index}: {case}"),
            );
        }
    }
}

/// 【知识库对照】【场景分组】每个场景重建旧文件，全部步骤通过实际公开工具执行
/// @param document 冻结场景文件
/// @returns 无；最终文件字节和每步成功或错误均匹配
async fn business(document: &str) {
    let root = tempfile::tempdir().unwrap();
    let (runtime, _) = runtime(root.path(), json!({}));
    let fixture: Value = serde_json::from_str(document).unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let kb = root.path().join("kb");
        if kb.exists() {
            std::fs::remove_dir_all(&kb).unwrap();
        }
        seed(root.path(), &case["files"], case["initialized"] != false);
        assert_eq!(case["expected"]["ok"], true, "native fixture {index}");
        for (step, expected) in case["steps"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"]["value"]["results"].as_array().unwrap())
        {
            assert_result(
                call(
                    &runtime,
                    root.path(),
                    step["tool"].as_str().unwrap(),
                    step["args"].clone(),
                    true,
                )
                .await,
                expected,
                &format!("case {index}: {step}"),
            );
        }
        assert_eq!(
            normalize(files(root.path())),
            case["expected"]["value"]["files"],
            "files {index}: {case}"
        );
    }
}

/// 【知识库对照】【搜索样本】读取已冻结的关键词、名称和分页场景
/// @returns 无；全部公开输出匹配
#[tokio::test]
async fn knowledge_matches_native_search_and_names() {
    business(include_str!("fixtures/knowledge_reference_01.json")).await;
}

/// 【知识库对照】【分页样本】覆盖末尾换行、CRLF、完整整数及替换范围
/// @returns 无；公开输出及正文保持一致
#[tokio::test]
async fn knowledge_matches_native_read_and_edits() {
    business(include_str!("fixtures/knowledge_reference_02.json")).await;
}

/// 【知识库对照】【变更样本】覆盖编辑、用途拒绝、删除和未初始化读取
/// @returns 无；最终文件与原版相同
#[tokio::test]
async fn knowledge_matches_native_mutations() {
    business(include_str!("fixtures/knowledge_reference_03.json")).await;
}
