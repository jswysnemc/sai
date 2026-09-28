import type { SettingsSearchEntry } from "./settings-search-types";

/** Git 分区字段索引。 */
export const GIT_SEARCH_ENTRIES: SettingsSearchEntry[] = [
  { anchor: "git.default_view_mode", section: "git", labelEn: "Default view", labelZh: "默认视图", keywords: ["scm.default_view_mode", "tree", "list", "树形", "列表"] },
  { anchor: "git.count_badge", section: "git", labelEn: "Count badge", labelZh: "数量角标", keywords: ["scm.count_badge", "badge", "角标"] },
  { anchor: "git.untracked_changes", section: "git", labelEn: "Untracked files", labelZh: "未跟踪文件", keywords: ["git.untracked_changes", "untracked", "未跟踪"] },
  { anchor: "git.auto_repository_detection", section: "git", labelEn: "Detect repositories", labelZh: "自动探测仓库", keywords: ["git.auto_repository_detection", "repository", "仓库"] },
  { anchor: "git.autofetch", section: "git", labelEn: "Fetch automatically", labelZh: "自动获取远端更新", keywords: ["git.autofetch", "fetch", "远端"] },
  { anchor: "git.detect_worktrees", section: "git", labelEn: "Detect worktrees", labelZh: "探测 worktree", keywords: ["git.detect_worktrees", "worktree"] },
  { anchor: "git.detect_worktrees_limit", section: "git", labelEn: "Worktree limit", labelZh: "worktree 数量上限", keywords: ["git.detect_worktrees_limit"] },
  { anchor: "git.enable_smart_commit", section: "git", labelEn: "Smart Commit", labelZh: "Smart Commit", keywords: ["git.enable_smart_commit", "智能提交"] },
  { anchor: "git.suggest_smart_commit", section: "git", labelEn: "Suggest Smart Commit", labelZh: "提示 Smart Commit", keywords: ["git.suggest_smart_commit"] },
  { anchor: "git.show_action_button", section: "git", labelEn: "Commit action button", labelZh: "提交操作按钮", keywords: ["git.show_action_button"] },
  { anchor: "git.branch_random_name", section: "git", labelEn: "Suggest branch names", labelZh: "建议分支名称", keywords: ["git.branch_random_name.enable", "branch", "分支"] },
  { anchor: "git.post_commit_command", section: "git", labelEn: "After committing", labelZh: "提交后动作", keywords: ["git.post_commit_command", "push", "sync", "推送", "同步"] },
  { anchor: "git.auto_commit_message_enabled", section: "git", labelEn: "Generate commit messages", labelZh: "生成提交说明", keywords: ["git.auto_commit_message_enabled", "commit message", "AI"] },
  { anchor: "git.auto_commit_message_model", section: "git", labelEn: "Commit message model", labelZh: "提交说明模型", keywords: ["git.auto_commit_message_model", "git.auto_commit_message_provider_id"] },
  { anchor: "git.confirm_sync", section: "git", labelEn: "Confirm sync", labelZh: "确认同步", keywords: ["git.confirm_sync"] },
  { anchor: "git.confirm_force_push", section: "git", labelEn: "Confirm force push", labelZh: "确认强制推送", keywords: ["git.confirm_force_push", "force push"] },
  { anchor: "git.confirm_empty_commits", section: "git", labelEn: "Confirm empty commits", labelZh: "确认空提交", keywords: ["git.confirm_empty_commits"] }
];
