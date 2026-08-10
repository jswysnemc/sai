use crate::llm::ChatStreamKind;
use std::sync::{Arc, Mutex};

/// 已经推送给用户但尚未落库的流式助手内容。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PartialTurnContent {
    pub content: String,
    pub reasoning: String,
}

/// 流式增量累积句柄。
///
/// 事件回调闭包与轮次守卫都需要触碰这份部分内容：闭包在流式过程中追加，
/// 守卫在收尾时读取。共享句柄让两者各自持有所有权，
/// 守卫因此不会被闭包借走，错误路径可以直接落终态而无需先解除借用。
#[derive(Debug, Default, Clone)]
pub struct PartialTurnSink {
    inner: Arc<Mutex<PartialTurnContent>>,
}

impl PartialTurnSink {
    /// 创建空的流式内容累积句柄。
    ///
    /// 返回:
    /// - 尚未累积任何内容的句柄
    pub fn new() -> Self {
        Self::default()
    }

    /// 累积一段已经发送给用户的流式内容。
    ///
    /// 参数:
    /// - `kind`: 流式内容类型
    /// - `text`: 本次增量文本
    ///
    /// 返回:
    /// - 无
    pub fn append(&self, kind: ChatStreamKind, text: &str) {
        let mut guard = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        match kind {
            ChatStreamKind::Content => guard.content.push_str(text),
            ChatStreamKind::Reasoning => guard.reasoning.push_str(text),
        }
    }

    /// 读取当前已累积的部分内容快照。
    ///
    /// 返回:
    /// - 部分正文与部分思考
    pub fn snapshot(&self) -> PartialTurnContent {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证正文与思考分别累积且互不串扰。
    #[test]
    fn appends_content_and_reasoning_separately() {
        let sink = PartialTurnSink::new();

        sink.append(ChatStreamKind::Content, "答复");
        sink.append(ChatStreamKind::Reasoning, "思考");
        sink.append(ChatStreamKind::Content, "续写");

        let snapshot = sink.snapshot();
        assert_eq!(snapshot.content, "答复续写");
        assert_eq!(snapshot.reasoning, "思考");
    }

    /// 验证克隆句柄与原句柄共享同一份累积内容。
    ///
    /// 事件闭包持有的是克隆件，守卫读取的是原件，二者必须一致，
    /// 否则中断落库会丢掉界面上已经显示的正文。
    #[test]
    fn clones_share_the_same_buffer() {
        let sink = PartialTurnSink::new();
        let cloned = sink.clone();

        cloned.append(ChatStreamKind::Content, "来自闭包");

        assert_eq!(sink.snapshot().content, "来自闭包");
    }
}
