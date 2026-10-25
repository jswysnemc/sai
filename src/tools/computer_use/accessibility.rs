//! 在已授权的最新窗口快照中检索控件，不借助全局桌面或任意脚本。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

/// 【桌面操作】【控件查询】读取同一快照的控件值、文本或匹配项，返回句柄而非自动选择目标。
/// 参数: `args` 为已校验的查询参数，`elements` 为公开句柄对应的快照内容；返回: 查询数据或缺失字段错误。
pub(super) fn query(args: &Value, elements: &HashMap<String, Value>) -> Result<Value> {
    // 1. 【桌面操作】【文本查找】仅检索当前快照的公开字段，不猜测或调用原生输入
    if args["action"] == "locate_text" {
        let name = args["name"].as_str().unwrap().to_lowercase();
        if name.trim().is_empty() {
            bail!("locate_text name must not be blank");
        }
        let mut matches = elements
            .iter()
            .filter(|(_, element)| {
                ["label", "value"].iter().any(|field| {
                    element[*field]
                        .as_str()
                        .is_some_and(|text| text.to_lowercase().contains(&name))
                })
            })
            .map(|(handle, element)| {
                let mut row = element.clone();
                row["element_index"] = json!(handle);
                row
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|element| {
            element["element_index"]
                .as_str()
                .and_then(|handle| handle.split_once(':'))
                .and_then(|(_, index)| index.parse::<usize>().ok())
                .unwrap_or(usize::MAX)
        });
        // 2. 【桌面操作】【匹配限长】保留顺序与总数，避免查询结果突破上下文预算
        let total = matches.len();
        let mut bytes = 0;
        let matches = matches
            .into_iter()
            .take(50)
            .take_while(|row| {
                bytes += row.to_string().len();
                bytes <= 32_000
            })
            .collect::<Vec<_>>();
        return Ok(
            json!({"truncated":matches.len() < total,"matches":matches,"total_matches":total}),
        );
    }
    // 3. 【桌面操作】【字段读取】通过当前句柄取值，缺失字段明确报错
    let handle = args["element_index"].as_str().unwrap();
    let element = elements
        .get(handle)
        .context("stale_element: copy an element_index from the current snapshot")?;
    let field = if args["action"] == "get_value" {
        "value"
    } else if element.get("value").is_some() {
        "value"
    } else {
        "label"
    };
    let value = element.get(field).context("requested control field is unavailable in this snapshot; observe again with a narrower query")?;
    let mut result = json!({"element_index":handle,"field":field,"value":value});
    if element["truncated"] == true {
        result["truncated"] = json!(true);
    }
    Ok(result)
}
