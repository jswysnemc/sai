use crate::render::transcript::AnsiLine;

/// 当前 live tail 已写入终端的预换行行快照。
#[derive(Default)]
pub(super) struct StreamState {
    rendered_lines: Vec<AnsiLine>,
}

/// live tail 与终端已写行的差异。
pub(super) enum StreamSync {
    Unchanged,
    Append(Vec<AnsiLine>),
    Rebuild,
}

impl StreamState {
    /// 用新 live tail 行计算可追加差异。
    ///
    /// 参数:
    /// - `next_lines`: 当前完整 live tail 的预换行行
    ///
    /// 返回:
    /// - 可增量追加或需要全量重建的差异
    pub(super) fn sync(&mut self, next_lines: Vec<AnsiLine>) -> StreamSync {
        if next_lines == self.rendered_lines {
            return StreamSync::Unchanged;
        }
        if next_lines.starts_with(&self.rendered_lines) {
            let appended = next_lines[self.rendered_lines.len()..].to_vec();
            self.rendered_lines = next_lines;
            return StreamSync::Append(appended);
        }
        self.rendered_lines = next_lines;
        StreamSync::Rebuild
    }

    /// 用一次完整重放后的 live tail 状态重置快照。
    ///
    /// 参数:
    /// - `lines`: 当前 live tail 的预换行行
    ///
    /// 返回:
    /// - 无
    pub(super) fn reset(&mut self, lines: Vec<AnsiLine>) {
        self.rendered_lines = lines;
    }
}

#[cfg(test)]
mod tests {
    use super::{StreamState, StreamSync};
    use crate::render::transcript::AnsiLine;

    /// 验证稳定前缀仅追加新增的完整行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn appends_only_new_stable_lines() {
        let mut state = StreamState::default();
        let first = vec![AnsiLine::new("first".to_string())];
        let second = vec![
            AnsiLine::new("first".to_string()),
            AnsiLine::new("second".to_string()),
        ];

        assert!(matches!(state.sync(first), StreamSync::Append(lines) if lines.len() == 1));
        assert!(
            matches!(state.sync(second), StreamSync::Append(lines) if lines == vec![AnsiLine::new("second".to_string())])
        );
    }

    /// 验证已写入行发生变化时要求从 source 完整重建。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn rebuilds_when_existing_line_changes() {
        let mut state = StreamState::default();
        state.reset(vec![AnsiLine::new("old".to_string())]);

        assert!(matches!(
            state.sync(vec![AnsiLine::new("new".to_string())]),
            StreamSync::Rebuild
        ));
    }
}
