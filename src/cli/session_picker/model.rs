use crate::state::ResumeTarget;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

/// 【会话选择】【列表状态】保存搜索、视口偏移和精确选中目标。
pub(super) struct Picker {
    pub targets: Vec<ResumeTarget>,
    pub current_workspace: String,
    pub all: bool,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    pub visible: Vec<usize>,
    pub loading: bool,
    pub load_error: Option<String>,
}

impl Picker {
    /// 【会话选择】【初始化】建立当前目录视图或显式全量视图。
    ///
    /// 参数:
    /// - `targets`: 目录快照
    /// - `current_workspace`: 当前工作区标识
    /// - `all`: 初始范围是否为全部工作区
    ///
    /// 返回:
    /// - 初始化后的选择器状态
    pub fn new(targets: Vec<ResumeTarget>, current_workspace: String, all: bool) -> Self {
        let mut state = Self {
            targets,
            current_workspace,
            all,
            query: String::new(),
            selected: 0,
            scroll: 0,
            visible: Vec::new(),
            loading: false,
            load_error: None,
        };
        state.filter();
        state
    }

    /// 【会话选择】【粘贴查询】剥离终端控制序列与换行后将文本追加到搜索框。
    ///
    /// 参数:
    /// - `text`: 粘贴原始文本
    ///
    /// 返回:
    /// - 查询发生变化时返回 true
    pub fn paste_query(&mut self, text: &str) -> bool {
        let cleaned: String = crate::cli::repl_text::strip_terminal_control_sequences(text)
            .chars()
            .filter_map(|ch| {
                if ch == '\t' {
                    Some(' ')
                } else if ch.is_control() {
                    None
                } else {
                    Some(ch)
                }
            })
            .collect();
        if cleaned.is_empty() {
            return false;
        }
        self.query.push_str(&cleaned);
        self.selected = 0;
        self.scroll = 0;
        self.filter();
        true
    }

    /// 【会话选择】【目录更新】应用后台快照，保留查询、范围与工作区内的精确选中身份。
    ///
    /// 参数:
    /// - `targets`: 最新会话目录
    ///
    /// 返回:
    /// - 无
    pub(super) fn replace_targets(&mut self, targets: Vec<ResumeTarget>) {
        // 1. 【会话选择】【精确身份】记录完整身份，避免新增工作区重排后选中其他同名会话
        let selected = self.target().map(|target| {
            (
                target.session.workspace_id.clone(),
                target.session.info.id.clone(),
            )
        });
        // 2. 【会话选择】【快照筛选】沿用当前查询和范围，再恢复仍然可见的目标
        self.targets = targets;
        self.filter();
        self.selected = selected
            .and_then(|(workspace, session)| {
                self.visible.iter().position(|index| {
                    let target = &self.targets[*index];
                    target.session.workspace_id == workspace && target.session.info.id == session
                })
            })
            .unwrap_or(0);
    }

    /// 【会话选择】【过滤】按标题、标识和路径匹配，保留工作区分组顺序。
    ///
    /// 参数:
    /// - `self`: 当前状态
    ///
    /// 返回:
    /// - 无
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

    /// 【会话选择】【范围切换】切换当前/全部工作区，保留仍然可见的精确目标。
    ///
    /// 参数:
    /// - `self`: 当前状态
    ///
    /// 返回:
    /// - 无
    pub fn toggle(&mut self) {
        let target = self.visible.get(self.selected).copied();
        self.all = !self.all;
        self.scroll = 0;
        self.filter();
        self.selected = target
            .and_then(|index| self.visible.iter().position(|value| *value == index))
            .unwrap_or(0);
    }

    /// 【会话选择】【面板高度】计算已加载会话展开后的行数（会话行 + 工作区标题行）。
    ///
    /// 参数:
    /// - `self`: 当前状态
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
    ///
    /// 参数:
    /// - `self`: 当前状态
    ///
    /// 返回:
    /// - 当前目标或空
    pub fn target(&self) -> Option<&ResumeTarget> {
        self.visible
            .get(self.selected)
            .map(|index| &self.targets[*index])
    }
}
