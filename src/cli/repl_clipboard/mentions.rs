use super::*;
use crate::cli::repl_mentions::{apply_mention, MentionKind, MentionSuggestion, MentionTrigger};

impl ReplClipboardState {
    /// 将确认的文件或技能引用登记为原子块，目录补全继续保留可编辑文本。
    ///
    /// 参数: `input` 为原输入，`trigger` 为补全范围，`item` 为选中的引用
    /// 返回: 替换后的输入及光标字符位置
    pub(in crate::cli) fn complete_mention(
        &mut self,
        input: &str,
        trigger: &MentionTrigger,
        item: &MentionSuggestion,
    ) -> (String, usize) {
        if item.continue_filter {
            return apply_mention(input, trigger, item);
        }
        let marker = format!("[{}]", item.insert.trim());
        let kind = match trigger.kind {
            MentionKind::File => ReplClipboardBlockKind::File,
            MentionKind::Skill => ReplClipboardBlockKind::Skill,
        };
        self.items.push(ReplClipboardItem::Reference {
            marker: marker.clone(),
            text: item.insert.trim().to_string(),
            kind,
        });
        let atom = MentionSuggestion {
            insert: marker,
            ..item.clone()
        };
        apply_mention(input, trigger, &atom)
    }
}
