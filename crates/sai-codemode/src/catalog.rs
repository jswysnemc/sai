use crate::declarations::tool_declaration;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub declaration: String,
}

impl CatalogEntry {
    /// 【CodeMode】【目录构造】把 name/description/parameters 转为目录项，返回声明完整的条目。
    pub fn new(name: String, description: String, parameters: Value) -> Self {
        let declaration = tool_declaration(&name, &parameters);
        Self {
            name,
            description,
            parameters,
            declaration,
        }
    }
}

/// 【CodeMode】【目录预算】按 token_budget 估算字符预算，返回完整条目组成的内联目录。
pub fn inline(entries: &[CatalogEntry], token_budget: usize) -> String {
    let mut output = String::new();
    for entry in entries {
        let description = entry.description.replace("*/", "* /");
        let item = format!("/** {description} */\n{}\n", entry.declaration);
        if output.chars().count() + item.chars().count() > token_budget.saturating_mul(4) {
            continue;
        }
        output.push_str(&item);
    }
    if output.is_empty() {
        return output;
    }
    format!("declare const tools: {{\n{output}}};\n")
}
