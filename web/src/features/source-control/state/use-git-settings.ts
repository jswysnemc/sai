import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import type { GitConfig, ScmConfig } from "../../../api/contracts";

export const DEFAULT_SCM_CONFIG: ScmConfig = {
  default_view_mode: "list",
  count_badge: "all"
};

export const DEFAULT_GIT_CONFIG: GitConfig = {
  auto_repository_detection: true,
  untracked_changes: "separate",
  enable_smart_commit: false,
  suggest_smart_commit: true,
  confirm_sync: true,
  confirm_force_push: true,
  confirm_empty_commits: true,
  post_commit_command: "none",
  show_action_button: true,
  detect_worktrees: true,
  detect_worktrees_limit: 10,
  autofetch: false,
  branch_random_name: { enable: false },
  auto_commit_message_enabled: true,
  auto_commit_message_provider_id: "",
  auto_commit_message_model: ""
};

/**
 * 读取 Source Control 使用的持久化 Git 配置。
 *
 * @returns Git、SCM 配置和读取状态
 */
export function useGitSettings() {
  const query = useQuery({
    queryKey: ["config"],
    queryFn: api.config.load,
    staleTime: 30_000
  });
  return {
    scm: query.data?.config.scm ?? DEFAULT_SCM_CONFIG,
    git: query.data?.config.git ?? DEFAULT_GIT_CONFIG,
    loading: query.isLoading,
    error: query.error
  };
}
