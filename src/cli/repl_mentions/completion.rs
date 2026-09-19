use super::worker::{Request, Response, Worker};
use super::{filter_skills, find_mention_trigger, MentionKind, MentionSuggestion};
use std::path::PathBuf;

#[cfg(test)]
mod tests;

/// 确认结果仍适用于当前输入所需的全部上下文
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub(super) input: String,
    pub(super) cursor: usize,
    pub(super) cwd: PathBuf,
}

/// 【终端】【异步补全】输入快照与后台任务生命周期
#[derive(Default)]
pub(in crate::cli) struct MentionCompletion {
    worker: Option<Worker>,
    snapshot: Option<Snapshot>,
    id: u64,
    items: Vec<MentionSuggestion>,
    pending: bool,
}

impl MentionCompletion {
    /// 返回当前输入对应的候选，input/cursor 为草稿，skills 为内存目录；不等待文件扫描
    pub(in crate::cli) fn query(
        &mut self,
        input: &str,
        cursor: usize,
        skills: &[(String, String)],
    ) -> Vec<MentionSuggestion> {
        let snapshot = Snapshot {
            input: input.into(),
            cursor,
            cwd: crate::runtime_cwd::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        if self.snapshot.as_ref() != Some(&snapshot) {
            self.cancel();
            self.snapshot = Some(snapshot.clone());
            match find_mention_trigger(input, cursor) {
                Some(trigger) if trigger.kind == MentionKind::Skill => {
                    self.items = filter_skills(skills, &trigger.query);
                }
                Some(trigger) => {
                    self.pending = true;
                    self.worker.get_or_insert_with(Worker::new).submit(Request {
                        id: self.id,
                        snapshot,
                        query: trigger.query,
                    });
                }
                None => {}
            }
        }
        self.poll();
        self.items.clone()
    }

    /// 收取后台结果；无参数，返回候选是否更新
    pub(in crate::cli) fn poll(&mut self) -> bool {
        self.worker
            .as_ref()
            .and_then(Worker::take)
            .is_some_and(|response| self.accept(response))
    }

    /// 校验 response 的序号与文本、光标、目录快照；返回是否接受结果
    fn accept(&mut self, response: Response) -> bool {
        if response.id != self.id || self.snapshot.as_ref() != Some(&response.snapshot) {
            return false;
        }
        self.items = response.items;
        self.pending = false;
        true
    }

    /// 取消当前查询并清理已显示结果，无参数，无返回值
    pub(in crate::cli) fn cancel(&mut self) {
        self.id = self.id.wrapping_add(1);
        if let Some(worker) = &self.worker {
            worker.cancel(self.id);
        }
        self.items.clear();
        self.snapshot = None;
        self.pending = false;
    }

    /// 返回是否等待后台扫描完成，无参数
    pub(in crate::cli) fn pending(&self) -> bool {
        self.pending
    }
}
