use anyhow::{ensure, Result};
use serde::Deserialize;

/// 【文档转换】【转换方式】明确选择原文、HTML 纯文本或 HTML Markdown，不推断内容类型
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mode {
    #[default]
    Raw,
    HtmlText,
    HtmlMarkdown,
}

/// 【文档转换】【输入选项】限制字符摘录、换行宽度和本次工作期限
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Options {
    #[serde(default)]
    pub mode: Mode,
    pub max_chars: Option<usize>,
    pub width: Option<usize>,
    pub timeout_ms: Option<u64>,
}

impl Options {
    /// 【文档转换】【选项校验】拒绝无意义数量和过大宽度，字符数还受实际输出预算限制
    /// @returns 参数有效时成功，调用者必须在开始原生工作前执行
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.max_chars != Some(0),
            "document max_chars must be positive"
        );
        ensure!(
            self.width.is_none_or(|width| (20..=200).contains(&width)),
            "document width must be between 20 and 200"
        );
        ensure!(
            self.timeout_ms != Some(0),
            "document timeout must be positive"
        );
        Ok(())
    }
}
