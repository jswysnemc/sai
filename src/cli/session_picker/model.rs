use crate::state::ResumeTarget;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

/// 【会话选择】【列表状态】保存搜索、视图和精确选中目标，绘制层不改变状态。
pub(super) struct Picker {
    pub targets: Vec<ResumeTarget>,
    pub current_workspace: String,
    pub all: bool,
    pub query: String,
    pub selected: usize,
    pub visible: Vec<usize>,
}

impl Picker {
    /// 【会话选择】【初始化】建立当前目录视图或显式全量视图。
    /// 参数: targets 为目录快照，current_workspace 为当前标识，all 为初始范围；返回: 状态
    pub fn new(targets: Vec<ResumeTarget>, current_workspace: String, all: bool) -> Self {
        let mut state = Self {
            targets,
            current_workspace,
            all,
            query: String::new(),
            selected: 0,
            visible: Vec::new(),
        };
        state.filter();
        state
    }

    /// 【会话选择】【过滤】按标题、标识和路径匹配，保留工作区分组顺序。
    /// 参数: 无；返回: 无
    pub fn filter(&mut self) {
        let matcher = SkimMatcherV2::default();
        self.visible = self
            .targets
            .iter()
            .enumerate()
            .filter(|(_, target)| {
                if !self.all && target.session.workspace_id != self.current_workspace {
                    return false;
                }
                let text = format!(
                    "{} {} {}",
                    target.session.info.title,
                    target.session.info.id,
                    target
                        .workspace_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_default()
                );
                self.query.is_empty() || matcher.fuzzy_match(&text, &self.query).is_some()
            })
            .map(|(index, _)| index)
            .collect();
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }

    /// 【会话选择】【范围切换】切换当前/全部，保留仍然可见的精确目标。
    /// 参数: 无；返回: 无
    pub fn toggle(&mut self) {
        let target = self.visible.get(self.selected).copied();
        self.all = !self.all;
        self.filter();
        self.selected = target
            .and_then(|index| self.visible.iter().position(|value| *value == index))
            .unwrap_or(0);
    }

    /// 【会话选择】【面板高度】计算全部会话展开后的行数（会话行 + 工作区标题行）。
    ///
    /// 面板高度据此固定，切换范围或输入搜索时外框不会跳动。
    ///
    /// 返回:
    /// - 全量列表的视觉行数
    pub fn max_rows(&self) -> usize {
        let workspaces = self
            .targets
            .iter()
            .map(|target| target.session.workspace_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        self.targets.len() + workspaces
    }

    /// 【会话选择】【当前目标】返回完整工作区与会话身份，空列表不可确认。
    /// 参数: 无；返回: 当前目标或空
    pub fn target(&self) -> Option<&ResumeTarget> {
        self.visible
            .get(self.selected)
            .map(|index| &self.targets[*index])
    }
}
