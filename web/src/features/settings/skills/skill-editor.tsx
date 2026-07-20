import { Eye, FileCode2, Save, Sparkles } from "lucide-react";
import { useMemo, useState } from "react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { MarkdownRenderer } from "../../chat/markdown-renderer";
import { EditorHeader, SettingsGroup } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";
import { composeSkillDocument, parseSkillDocument } from "./skill-document";
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
 * 编辑新建或已安装 Skill 的完整 SKILL.md。
 * 名称与描述独立字段展示，正文区放大编辑，并支持 Markdown 预览。
 *
 * @param props 当前条目、文档内容、保存状态与更新回调
 * @returns Skill 文档编辑区
 */
export function SkillEditor(props: SkillEditorProps) {
  const { t } = useI18n();
  const { skill, content, directoryName, creating, dirty, saving, error } = props;
  const [view, setView] = useState<EditorView>("edit");
  const parsed = useMemo(() => parseSkillDocument(content), [content]);

  /**
   * 更新 frontmatter 字段或正文后回写完整文档。
   *
   * @param patch 局部更新
   */
  const updateDocument = (patch: Partial<{ name: string; description: string; body: string }>) => {
    props.onContentChange(composeSkillDocument(
      patch.name ?? parsed.name,
      patch.description ?? parsed.description,
      patch.body ?? parsed.body
    ));
  };

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
            description={t("Name and description are saved in YAML frontmatter; body is Markdown.", "名称与描述写入 YAML frontmatter，正文为 Markdown。")}
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

            <div className="skill-meta-grid">
              <label className="settings-field skill-name-field">
                <span>{t("Name", "名称")}</span>
                <input
                  value={parsed.name}
                  onChange={(event) => updateDocument({ name: event.target.value })}
                  placeholder="code-review"
                  spellCheck={false}
                  aria-label={t("Skill name", "Skill 名称")}
                />
              </label>
              <label className="settings-field skill-description-field">
                <span>{t("Description", "描述")}</span>
                <textarea
                  value={parsed.description}
                  onChange={(event) => updateDocument({ description: event.target.value })}
                  placeholder={t("When should this Skill be used?", "何时应使用该 Skill？")}
                  rows={3}
                  aria-label={t("Skill description", "Skill 描述")}
                />
              </label>
            </div>

            <div className="skill-doc-toolbar">
              <div className="skill-view-tabs" role="tablist" aria-label={t("Document view", "文档视图")}>
                <Button
                  className={`settings-secondary${view === "edit" ? " active" : ""}`}
                  onClick={() => setView("edit")}
                  aria-selected={view === "edit"}
                >
                  <FileCode2 size={13} />
                  {t("Edit body", "编辑正文")}
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
              <span className="skill-doc-hint">SKILL.md · body</span>
            </div>

            {view === "edit" ? (
              <label className="settings-field full skill-content-field">
                <span className="sr-only">{t("Skill Markdown body", "Skill Markdown 正文")}</span>
                <textarea
                  value={parsed.body}
                  onChange={(event) => updateDocument({ body: event.target.value })}
                  spellCheck={false}
                  aria-label={t("Skill Markdown body", "Skill Markdown 正文")}
                />
              </label>
            ) : (
              <div className="skill-markdown-preview" aria-label={t("Skill Markdown preview", "Skill Markdown 预览")}>
                <header className="skill-preview-meta">
                  <strong>{parsed.name || t("Unnamed skill", "未命名 Skill")}</strong>
                  <p>{parsed.description || t("No description", "暂无描述")}</p>
                </header>
                {parsed.body.trim()
                  ? <MarkdownRenderer source={parsed.body} />
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
