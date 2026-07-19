import { GitBranch, ShieldCheck } from "lucide-react";
import type { AppConfig, GitConfig, ScmConfig } from "../../../api/contracts";
import { EditorHeader, SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";
import { DEFAULT_GIT_CONFIG, DEFAULT_SCM_CONFIG } from "../../source-control/state/use-git-settings";
import { GitSettingNumber, GitSettingSelect, GitSettingToggle } from "./git-settings-fields";
import "./git-settings-panel.css";

type GitSettingsPanelProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 Git 与 Source Control 的持久化设置。
 *
 * @param props 应用配置和更新回调
 * @returns Git 设置编辑区
 */
export function GitSettingsPanel(props: GitSettingsPanelProps) {
  const { t } = useI18n();
  const scm = props.config.scm ?? DEFAULT_SCM_CONFIG;
  const git = props.config.git ?? DEFAULT_GIT_CONFIG;

  /**
   * 更新 Source Control 显示配置。
   *
   * @param patch Source Control 字段补丁
   * @returns 无返回值
   */
  const updateScm = (patch: Partial<ScmConfig>) => {
    props.onConfigChange({ ...props.config, scm: { ...scm, ...patch } });
  };

  /**
   * 更新 Git 行为配置。
   *
   * @param patch Git 字段补丁
   * @returns 无返回值
   */
  const updateGit = (patch: Partial<GitConfig>) => {
    props.onConfigChange({ ...props.config, git: { ...git, ...patch } });
  };

  return (
    <div className="settings-editor git-settings-panel">
      <EditorHeader
        kicker="Source Control"
        title={t("Git and Source Control", "Git 与源代码管理")}
        description={t("Configure repository detection, change presentation, commits, remote operations, and confirmations.", "配置仓库探测、变更展示、提交、远端操作和确认行为。")}
      />
      <SettingsGroup title={t("Changes view", "变更视图")} description={t("Control the default file layout and Source Control count badge.", "控制默认文件布局和源代码管理数量角标。")}>
        <div className="git-settings-grid">
          <GitSettingSelect
            label={t("Default view mode", "默认视图模式")}
            description="scm.default_view_mode"
            value={scm.default_view_mode}
            options={[
              { value: "list", label: t("List", "列表") },
              { value: "tree", label: t("Tree", "树形") }
            ]}
            onChange={(value) => updateScm({ default_view_mode: value })}
          />
          <GitSettingSelect
            label={t("Count badge", "数量角标")}
            description="scm.count_badge"
            value={scm.count_badge}
            options={[
              { value: "all", label: t("All repositories", "全部仓库") },
              { value: "focused", label: t("Focused repository", "当前仓库") },
              { value: "off", label: t("Hidden", "隐藏") }
            ]}
            onChange={(value) => updateScm({ count_badge: value })}
          />
          <GitSettingSelect
            label={t("Untracked changes", "未跟踪变更")}
            description="git.untracked_changes"
            value={git.untracked_changes}
            options={[
              { value: "separate", label: t("Separate section", "独立分区") },
              { value: "mixed", label: t("Mix with changes", "合并到变更") },
              { value: "hidden", label: t("Hidden", "隐藏") }
            ]}
            onChange={(value) => updateGit({ untracked_changes: value })}
          />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Repository detection", "仓库探测")} description={t("Limit filesystem scanning and worktree discovery in large workspaces.", "限制大型工作区中的文件系统扫描和 worktree 探测。")}>
        <div className="git-settings-grid toggles">
          <GitSettingToggle label={t("Automatic repository detection", "自动探测仓库")} description="git.auto_repository_detection" checked={git.auto_repository_detection} onChange={(value) => updateGit({ auto_repository_detection: value })} />
          <GitSettingToggle label={t("Detect worktrees", "探测 worktree")} description="git.detect_worktrees" checked={git.detect_worktrees} onChange={(value) => updateGit({ detect_worktrees: value })} />
          <GitSettingToggle label={t("Automatic fetch", "自动获取远端更新")} description="git.autofetch" checked={git.autofetch} onChange={(value) => updateGit({ autofetch: value })} />
          <GitSettingNumber label={t("Worktree detection limit", "worktree 探测上限")} description="git.detect_worktrees_limit" value={git.detect_worktrees_limit} min={1} max={128} onChange={(value) => updateGit({ detect_worktrees_limit: value })} />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Commit workflow", "提交流程")} description={t("Configure Smart Commit, post-commit behavior, and action visibility.", "配置 Smart Commit、提交后动作和操作按钮显示。")}>
        <div className="git-settings-grid toggles">
          <GitSettingToggle label={t("Enable Smart Commit", "启用 Smart Commit")} description="git.enable_smart_commit" checked={git.enable_smart_commit} onChange={(value) => updateGit({ enable_smart_commit: value })} />
          <GitSettingToggle label={t("Suggest Smart Commit", "提示 Smart Commit")} description="git.suggest_smart_commit" checked={git.suggest_smart_commit} onChange={(value) => updateGit({ suggest_smart_commit: value })} />
          <GitSettingToggle label={t("Show commit action button", "显示提交操作按钮")} description="git.show_action_button" checked={git.show_action_button} onChange={(value) => updateGit({ show_action_button: value })} />
          <GitSettingSelect
            label={t("Post-commit command", "提交后命令")}
            description="git.post_commit_command"
            value={git.post_commit_command}
            options={[
              { value: "none", label: t("None", "无") },
              { value: "push", label: t("Push", "推送") },
              { value: "sync", label: t("Sync", "同步") }
            ]}
            onChange={(value) => updateGit({ post_commit_command: value })}
          />
          <GitSettingToggle
            label={t("Generate branch name suggestions", "生成分支名称建议")}
            description="git.branch_random_name.enable"
            checked={git.branch_random_name.enable}
            onChange={(value) => updateGit({ branch_random_name: { enable: value } })}
          />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("Safety confirmations", "安全确认")} description={t("Keep explicit confirmation around remote rewriting, sync, and empty commits.", "为远端改写、同步和空提交保留明确确认。")}>
        <div className="git-settings-grid toggles">
          <GitSettingToggle label={t("Confirm sync", "确认同步")} description="git.confirm_sync" checked={git.confirm_sync} onChange={(value) => updateGit({ confirm_sync: value })} />
          <GitSettingToggle label={t("Confirm force push", "确认强制推送")} description="git.confirm_force_push" checked={git.confirm_force_push} onChange={(value) => updateGit({ confirm_force_push: value })} />
          <GitSettingToggle label={t("Confirm empty commits", "确认空提交")} description="git.confirm_empty_commits" checked={git.confirm_empty_commits} onChange={(value) => updateGit({ confirm_empty_commits: value })} />
        </div>
        <div className="git-settings-safety-note"><ShieldCheck size={14} /><span>{t("Discard, hard reset, branch deletion, and worktree removal confirmations remain mandatory.", "丢弃、硬重置、删除分支和移除 worktree 的确认始终保留。")}</span></div>
      </SettingsGroup>
      <div className="git-settings-footer"><GitBranch size={14} />{t("Settings apply to the Source Control panel after saving.", "保存后设置会应用到源代码管理面板。")}</div>
    </div>
  );
}
