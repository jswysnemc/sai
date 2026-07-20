import { Eye, FileCode2, Save, Sparkles } from "lucide-react";
import { useState } from "react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { MarkdownRenderer } from "../../chat/markdown-renderer";
import { EditorHeader, SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";
import "../../chat/markdown-renderer.css";

type SkillEditorProps = {
  skill: ManagedSkill | null;
  content: string;
  directoryName: string;
  creating: boolean;
  dirty: boolean;
  saving: boolean;
  error: string | null;
  onContentChange: (content: string) => void;
  onDirectoryNameChange: (name: string) => void;
  onEnabledChange: (enabled: boolean) => void;
  onSave: () => void;
};

type EditorView = "edit" | "preview";

/**
 * 编辑新建或已安装 Skill 的完整 SKILL.md，支持源码编辑与 Markdown 预览。
 *
 * @param props 当前条目、文档内容、保存状态与更新回调
 * @returns Skill 文档编辑区
 */
export function SkillEditor(props: SkillEditorProps) {
  const { t } = useI18n();
  const { skill, content, directoryName, creating, dirty, saving, error } = props;
  const [view, setView] = useState<EditorView>("edit");

  return (
    <section className="settings-editor skill-editor">
      <EditorHeader
        kicker="Skills"
        title={creating ? t("New Skill", "新增 Skill") : (skill?.name ?? t("Skill manager", "Skill 管理"))}
        description={creating
          ? t("Create a global Skill directory with a complete SKILL.md document.", "在全局目录新增 Skill，并写入完整 SKILL.md。")
          : (skill ? skill.path : t("Select a Skill to inspect or edit its document.", "选择 Skill 后查看或编辑文档。"))}
        actions={skill && !creating ? (
          <label className="settings-switch">
            <input type="checkbox" checked={skill.enabled} onChange={(event) => props.onEnabledChange(event.target.checked)} />
            <span />
            <strong>{skill.enabled ? t("Enabled", "已启用") : t("Disabled", "已禁用")}</strong>
          </label>
        ) : undefined}
      />

      {error && <div className="settings-inline-error">{error}</div>}
      {!creating && !skill ? (
        <div className="settings-empty">
          <Sparkles size={20} />
          <p>{t("Scan or select a Skill to manage it.", "扫描或选择一个 Skill 进行管理。")}</p>
        </div>
      ) : (
        <>
          <SettingsGroup
            title={t("Skill document", "Skill 文档")}
            description={t("The name and description must exist in YAML frontmatter.", "YAML frontmatter 中必须包含 name 和 description。")}
          >
            {creating && (
              <label className="settings-field skill-directory-field">
                <span>{t("Directory name", "目录名称")}</span>
                <input
                  value={directoryName}
                  onChange={(event) => props.onDirectoryNameChange(event.target.value)}
                  placeholder="code-review"
                  spellCheck={false}
                />
                <small>{t("Letters, numbers, hyphens, and underscores only", "仅支持字母、数字、连字符和下划线")}</small>
              </label>
            )}

            <div className="skill-doc-toolbar">
              <div className="skill-view-tabs" role="tablist" aria-label={t("Document view", "文档视图")}>
                <Button
                  className={`settings-secondary${view === "edit" ? " active" : ""}`}
                  onClick={() => setView("edit")}
                  aria-selected={view === "edit"}
                >
                  <FileCode2 size={13} />
                  {t("Edit", "编辑")}
                </Button>
                <Button
                  className={`settings-secondary${view === "preview" ? " active" : ""}`}
                  onClick={() => setView("preview")}
                  aria-selected={view === "preview"}
                >
                  <Eye size={13} />
                  {t("Preview", "预览")}
                </Button>
              </div>
              <span className="skill-doc-hint">SKILL.md</span>
            </div>

            {view === "edit" ? (
              <label className="settings-field full skill-content-field">
                <span className="sr-only">SKILL.md</span>
                <textarea
                  value={content}
                  onChange={(event) => props.onContentChange(event.target.value)}
                  spellCheck={false}
                  aria-label={t("Skill Markdown document", "Skill Markdown 文档")}
                />
              </label>
            ) : (
              <div className="skill-markdown-preview" aria-label={t("Skill Markdown preview", "Skill Markdown 预览")}>
                {content.trim()
                  ? <MarkdownRenderer source={content} />
                  : <div className="skill-preview-empty">{t("Nothing to preview yet", "暂无预览内容")}</div>}
              </div>
            )}
          </SettingsGroup>
          <div className="skill-editor-actions">
            <span>{skill ? `${skill.scope} / ${skill.directory_name}` : t("Global Skill", "全局 Skill")}</span>
            <Button className="settings-secondary" disabled={!dirty || saving} onClick={props.onSave}>
              <Save size={14} />
              {saving ? t("Saving", "正在保存") : t("Save Skill", "保存 Skill")}
            </Button>
          </div>
        </>
      )}
    </section>
  );
}
