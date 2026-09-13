use serde::{Deserialize, Serialize};

/// 输入历史中的附件种类，独立于终端显示样式。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputHistoryAttachmentKind {
    Text,
    Image,
    File,
    Skill,
}

/// 一个原子块的标签及完整数据；图片数据保存在 content 中。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InputHistoryAttachment {
    pub marker: String,
    pub content: String,
    pub kind: InputHistoryAttachmentKind,
}

/// 可独立恢复的输入快照，普通文字与附件一起保存。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct InputHistoryEntry {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<InputHistoryAttachment>,
}

impl From<String> for InputHistoryEntry {
    /// 将普通文本转换为不含附件的历史条目。
    /// 参数: `text` 为原始文字
    /// 返回: 可持久化的输入快照
    fn from(text: String) -> Self {
        Self {
            text,
            attachments: Vec::new(),
        }
    }
}

impl AsRef<str> for InputHistoryEntry {
    /// 返回用于历史浏览比较的输入文字。
    /// 参数: 无
    /// 返回: 原始输入文字引用
    fn as_ref(&self) -> &str {
        &self.text
    }
}
