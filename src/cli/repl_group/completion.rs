//! 群组输入框补全计算：统一提供 `/` 斜杠命令补全与 `@` 成员提及补全（附带成员模型提示）。

use super::model_info::short_member_model;
use super::state::GroupTuiState;
use super::text_util::member_status_label;
use crate::cli::repl_text::byte_index_for_char;
use crate::super_agent::model::MemberRole;

/// 输入框补全候选项（支持 `@成员` 与 `/命令` 两种自动补全）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompletionItem {
    pub(super) insert_text: String,
    pub(super) label: String,
    pub(super) category: &'static str,
    pub(super) detail: String,
}

impl GroupTuiState {
    /// 【群组终端】【补全候选】计算当前输入框对应的 `/` 命令或 `@` 成员补全候选列表。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 候选补全条目（无补全触发或已被 Esc 收起时为空）
    pub(super) fn completion_items(&self) -> Vec<CompletionItem> {
        if self.modal.is_some() || self.completions_dismissed || self.input.is_empty() {
            return Vec::new();
        }
        let cursor_byte = byte_index_for_char(&self.input, self.cursor);
        let before_cursor = &self.input[..cursor_byte];

        // 1. 开头斜杠命令补全（仅在输入首词且无换行时触发）
        if before_cursor.starts_with('/')
            && !before_cursor.contains(char::is_whitespace)
            && !self.input.contains('\n')
        {
            let prefix = before_cursor.to_ascii_lowercase();
            let commands = [
                ("/model", "模型", "切换主模型、思考等级与上下文窗口"),
                ("/spawn ", "执行", "创建并启动 Runner：/spawn <名> <任务>"),
                ("/new ", "群组", "新建协作群组：/new <名称>"),
                ("/bind ", "渠道", "绑定外部渠道：/bind <weixin|feishu|qq> [chat_id|*]"),
                ("/gateway ", "渠道", "查看或启停后台网关：/gateway [start|stop] [weixin|qq]"),
                ("/channels", "检视", "切换到右侧「渠道」页查看微信/QQ/飞书接入"),
                ("/tools", "检视", "切换到右侧「工具」页查看 Leader/Runner 工具边界"),
                ("/sim ", "调试", "本地模拟渠道消息：/sim <渠道> <chat_id> <内容>"),
                ("/watchdog", "守护", "立即执行一次 Watchdog 健康巡检"),
                ("/help", "帮助", "打开快捷键、渠道接入与工具说明"),
                ("/quit", "退出", "退出群组工作台返回主对话"),
            ];
            let matches: Vec<CompletionItem> = commands
                .into_iter()
                .filter(|(cmd, _, _)| cmd.trim_end().starts_with(&prefix))
                .map(|(cmd, category, detail)| CompletionItem {
                    insert_text: cmd.to_string(),
                    label: cmd.trim_end().to_string(),
                    category,
                    detail: detail.to_string(),
                })
                .collect();
            // 仅剩 1 项且用户已完整输入该独立命令时自动收起，避免补全完成后仍遮挡底栏
            if matches.len() == 1 && matches[0].insert_text.eq_ignore_ascii_case(before_cursor) {
                return Vec::new();
            }
            return matches;
        }

        // 2. 光标所在词以 `@` 开头时触发群组成员补全
        let token_start = before_cursor
            .rfind(char::is_whitespace)
            .map(|pos| pos + 1)
            .unwrap_or(0);
        let token = &before_cursor[token_start..];
        if let Some(query) = token.strip_prefix('@') {
            let q = query.to_ascii_lowercase();
            let mut items = Vec::new();
            if let Some(detail) = &self.detail {
                for member in &detail.members {
                    if member.name.to_ascii_lowercase().starts_with(&q) {
                        let role = if member.role == MemberRole::Leader {
                            "Leader"
                        } else {
                            "Runner"
                        };
                        let model = if member.role == MemberRole::Leader {
                            self.model_info.leader.short_model.clone()
                        } else {
                            let short = short_member_model(&member.model_label);
                            if short.is_empty() {
                                self.model_info.worker.short_model.clone()
                            } else {
                                short
                            }
                        };
                        items.push(CompletionItem {
                            insert_text: format!("@{} ", member.name),
                            label: format!("@{}", member.name),
                            category: role,
                            detail: format!("{model} · {}", member_status_label(member.status)),
                        });
                    }
                }
            }
            if "all".starts_with(&q) {
                items.push(CompletionItem {
                    insert_text: "@all ".to_string(),
                    label: "@all".to_string(),
                    category: "广播",
                    detail: "由 Leader 统筹全体成员".to_string(),
                });
            }
            return items;
        }
        Vec::new()
    }

    /// 将当前选中的补全项应用到输入框并推进字符光标。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 命中并应用补全项时返回 true
    pub(super) fn apply_active_completion(&mut self) -> bool {
        self.apply_completion_at(self.completion_idx)
    }

    /// 将指定索引的补全项应用到输入框并推进字符光标。
    ///
    /// 参数:
    /// - `index`: 候选项下标
    ///
    /// 返回:
    /// - 命中并应用补全项时返回 true
    pub(super) fn apply_completion_at(&mut self, index: usize) -> bool {
        let items = self.completion_items();
        if items.is_empty() {
            return false;
        }
        let chosen = &items[index.min(items.len() - 1)];
        let cursor_byte = byte_index_for_char(&self.input, self.cursor);
        let before = &self.input[..cursor_byte];
        let after = &self.input[cursor_byte..];
        let replace_start = if before.starts_with('/') && !before.contains(char::is_whitespace) {
            0
        } else {
            before
                .rfind(char::is_whitespace)
                .map(|pos| pos + 1)
                .unwrap_or(0)
        };
        let mut next = String::with_capacity(self.input.len() + chosen.insert_text.len());
        next.push_str(&self.input[..replace_start]);
        next.push_str(&chosen.insert_text);
        let next_cursor_chars = next.chars().count();
        next.push_str(after);
        self.input = next;
        self.cursor = next_cursor_chars;
        self.completion_idx = 0;
        self.completions_dismissed = false;
        true
    }
}
