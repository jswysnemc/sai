import type { AppConfig, GitConfig, ScmConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { DEFAULT_GIT_CONFIG, DEFAULT_SCM_CONFIG } from "../../source-control/state/use-git-settings";
import { ModelChoiceSelect } from "../controls/model-choice-select";
import {
  ChoicePills,
  FieldGrid,
  InlineNotice,
  SettingsField,
  SettingsPanel,
  SkNumberInput,
  SwitchField
} from "../kit";

type GitSettingsPanelProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 Git 与源代码管理设置：变更视图、仓库探测、提交流程与安全确认。
 *
 * @param props 应用配置和更新回调
 * @returns Git 设置页
 */
export function GitSettingsPanel({ config, onConfigChange }: GitSettingsPanelProps) {
  const { t } = useI18n();
  const scm = config.scm ?? DEFAULT_SCM_CONFIG;
  const git = config.git ?? DEFAULT_GIT_CONFIG;

  /**
   * 更新源代码��理显示配置。
   *
   * @param patch 字段补丁
   * @returns 无返回值
   */
  const updateScm = (patch: Partial<ScmConfig>) => {
    onConfigChange({ ...config, scm: { ...scm, ...patch } });
  };

  /**
   * 更新 Git 行为配置。
   *
   * @param patch 字段补丁
   * @returns 无返回值
   */
  const updateGit = (patch: Partial<GitConfig>) => {
    onConfigChange({ ...config, git: { ...git, ...patch } });
  };

  return (
    <>
      <SettingsPanel
        title={t("Changes view", "变更视图")}
        description={t("How the Source Control panel lists changed files.", "源代码管理面板如何列出变更文件。")}
      >
        <FieldGrid columns={3}>
          <SettingsField
            label={t("Default view", "默认视图")}
            hint={t("File layout when the panel opens.", "面板打开时的文件排布。")}
            configKey="scm.default_view_mode"
            anchor="git.default_view_mode"
          >
            <ChoicePills
              value={scm.default_view_mode}
              options={[
                { value: "list", label: t("List", "列表") },
                { value: "tree", label: t("Tree", "树形") }
              ]}
              onChange={(value) => updateScm({ default_view_mode: value })}
            />
          </SettingsField>
          <SettingsField
            label={t("Count badge", "数量角标")}
            hint={t("Which repositories count toward the badge on the Source Control icon.", "源代码管理图标上的变更计数范围。")}
            configKey="scm.count_badge"
            anchor="git.count_badge"
          >
            <ChoicePills
              value={scm.count_badge}
              options={[
                { value: "all", label: t("All", "全部仓库") },
                { value: "focused", label: t("Focused", "当前仓库") },
                { value: "off", label: t("Hidden", "隐藏") }
              ]}
              onChange={(value) => updateScm({ count_badge: value })}
            />
          </SettingsField>
          <SettingsField
            label={t("Untracked files", "未跟踪文件")}
            hint={t("Where new files that Git does not track yet appear.", "Git 尚未跟踪的新文件显示在哪里。")}
            configKey="git.untracked_changes"
            anchor="git.untracked_changes"
          >
            <ChoicePills
              value={git.untracked_changes}
              options={[
                { value: "separate", label: t("Own group", "独立分组") },
                { value: "mixed", label: t("With changes", "并入变更") },
                { value: "hidden", label: t("Hidden", "隐藏") }
              ]}
              onChange={(value) => updateGit({ untracked_changes: value })}
            />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>

      <SettingsPanel
        title={t("Repository detection", "仓库探测")}
        description={t("Limit filesystem scanning and worktree discovery in large workspaces.", "限制大型工作区中的文件扫描与 worktree 探测。")}
      >
        <FieldGrid>
          <SwitchField
            label={t("Detect repositories", "自动探测仓库")}
            hint={t("Find Git repositories when a workspace opens.", "打开工作区时自动识别其中的 Git 仓库。")}
            configKey="git.auto_repository_detection"
            anchor="git.auto_repository_detection"
            checked={git.auto_repository_detection}
            onChange={(value) => updateGit({ auto_repository_detection: value })}
          />
          <SwitchField
            label={t("Fetch automatically", "自动获取远端更新")}
            hint={t("Run git fetch periodically so remote branch status stays current.", "定期执行 git fetch，保持远端分支状态最新。")}
            configKey="git.autofetch"
            anchor="git.autofetch"
            checked={git.autofetch}
            onChange={(value) => updateGit({ autofetch: value })}
          />
          <SwitchField
            label={t("Detect worktrees", "探测 worktree")}
            hint={t("Also list the worktrees linked to each repository.", "同时列出仓库关联的 worktree。")}
            configKey="git.detect_worktrees"
            anchor="git.detect_worktrees"
            checked={git.detect_worktrees}
            onChange={(value) => updateGit({ detect_worktrees: value })}
          />
          <SettingsField
            label={t("Worktree limit", "worktree 数量上限")}
            hint={t("Maximum worktrees listed per repository, 1 to 128.", "每个仓库最多列出的 worktree 数量，范围 1 到 128。")}
            configKey="git.detect_worktrees_limit"
            anchor="git.detect_worktrees_limit"
            size="xs"
          >
            <SkNumberInput
              value={git.detect_worktrees_limit}
              min={1}
              max={128}
              integer
              disabled={!git.detect_worktrees}
              onChange={(value) => updateGit({ detect_worktrees_limit: value ?? DEFAULT_GIT_CONFIG.detect_worktrees_limit })}
            />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>

      <SettingsPanel
        title={t("Commit workflow", "提交流程")}
        description={t("Smart Commit, commit messages, and what happens after a commit.", "Smart Commit、提交说明，以及提交之后的动作。")}
      >
        <FieldGrid>
          <SwitchField
            label={t("Smart Commit", "Smart Commit")}
            hint={t("Commit every change when nothing is staged.", "暂存区为空时，提交全部变更。")}
            configKey="git.enable_smart_commit"
            anchor="git.enable_smart_commit"
            checked={git.enable_smart_commit}
            onChange={(value) => updateGit({ enable_smart_commit: value })}
          />
          <SwitchField
            label={t("Suggest Smart Commit", "提示 Smart Commit")}
            hint={t("Ask before committing everything when nothing is staged.", "暂存区为空时先询问是否提交全部变更。")}
            configKey="git.suggest_smart_commit"
            anchor="git.suggest_smart_commit"
            checked={git.suggest_smart_commit}
            onChange={(value) => updateGit({ suggest_smart_commit: value })}
          />
          <SwitchField
            label={t("Commit action button", "提交操作按钮")}
            hint={t("Show the primary commit button under the message box.", "在提交说明输入框下方显示提交主按钮。")}
            configKey="git.show_action_button"
            anchor="git.show_action_button"
            checked={git.show_action_button}
            onChange={(value) => updateGit({ show_action_button: value })}
          />
          <SwitchField
            label={t("Suggest branch names", "建议分支名称")}
            hint={t("Prefill a random name when creating a branch.", "新建分支时预填随机名称。")}
            configKey="git.branch_random_name.enable"
            anchor="git.branch_random_name"
            checked={git.branch_random_name.enable}
            onChange={(value) => updateGit({ branch_random_name: { enable: value } })}
          />
          <SettingsField
            label={t("After committing", "提交后动作")}
            hint={t("Remote operation to run after a successful commit.", "提交成功后自动执行的远端操作。")}
            configKey="git.post_commit_command"
            anchor="git.post_commit_command"
          >
            <ChoicePills
              value={git.post_commit_command}
              options={[
                { value: "none", label: t("Nothing", "不执行") },
                { value: "push", label: t("Push", "推送") },
                { value: "sync", label: t("Sync", "同步") }
              ]}
              onChange={(value) => updateGit({ post_commit_command: value })}
            />
          </SettingsField>
          <SwitchField
            label={t("Generate commit messages", "生成提交说明")}
            hint={t("Draft the commit message from the staged changes.", "根据暂存的变更起草提交说明。")}
            configKey="git.auto_commit_message_enabled"
            anchor="git.auto_commit_message_enabled"
            checked={git.auto_commit_message_enabled ?? true}
            onChange={(value) => updateGit({ auto_commit_message_enabled: value })}
          />
          <SettingsField
            label={t("Commit message model", "提交说明模型")}
            hint={t("Model used to draft commit messages.", "起草提交说明使用的模型。")}
            configKey="git.auto_commit_message_model"
            anchor="git.auto_commit_message_model"
            size="lg"
          >
            <ModelChoiceSelect
              config={config}
              providerId={git.auto_commit_message_provider_id}
              model={git.auto_commit_message_model}
              inheritLabel={t("Active model", "当前模型")}
              inheritDescription={t("Use the model of the active provider.", "使用当前供应商的模型。")}
              optionDescription={t("Always use this model for commit messages", "始终使用该模型生成提交说明")}
              onChange={(providerId, model) => updateGit({
                auto_commit_message_provider_id: providerId,
                auto_commit_message_model: model
              })}
            />
          </SettingsField>
        </FieldGrid>
      </SettingsPanel>

      <SettingsPanel
        title={t("Safety confirmations", "安全确认")}
        description={t("Ask before operations that rewrite remote history or record empty commits.", "改写远端历史或记录空提交前先确认。")}
      >
        <FieldGrid columns={3}>
          <SwitchField
            label={t("Confirm sync", "确认同步")}
            hint={t("Before pulling and pushing in one step.", "一次性拉取并推送之前。")}
            configKey="git.confirm_sync"
            anchor="git.confirm_sync"
            checked={git.confirm_sync}
            onChange={(value) => updateGit({ confirm_sync: value })}
          />
          <SwitchField
            label={t("Confirm force push", "确认强制推送")}
            hint={t("Before overwriting the remote branch.", "覆盖远端分支之前。")}
            configKey="git.confirm_force_push"
            anchor="git.confirm_force_push"
            checked={git.confirm_force_push}
            onChange={(value) => updateGit({ confirm_force_push: value })}
          />
          <SwitchField
            label={t("Confirm empty commits", "确认空提交")}
            hint={t("Before committing with no changes.", "提交没有变更的内容之前。")}
            configKey="git.confirm_empty_commits"
            anchor="git.confirm_empty_commits"
            checked={git.confirm_empty_commits}
            onChange={(value) => updateGit({ confirm_empty_commits: value })}
          />
        </FieldGrid>
        <InlineNotice>
          {t(
            "Discarding changes, hard reset, deleting branches, and removing worktrees always ask for confirmation.",
            "丢弃变更、硬重置、删除分支和移除 worktree 始终需要确认，不受以上开关影响。"
          )}
        </InlineNotice>
      </SettingsPanel>
    </>
  );
}
