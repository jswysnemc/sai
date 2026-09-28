import { BookOpen, Save } from "../../../shared/ui/icons";
import { useMemo } from "react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { MarkdownEditor } from "../../../shared/ui/markdown-editor/markdown-editor";
import { MarkdownModeToggle } from "../../../shared/ui/markdown-editor/markdown-mode-toggle";
import { useMarkdownMode } from "../../../shared/ui/markdown-editor/use-markdown-mode";
import { isDarkTheme, useTheme } from "../../theme/theme";
import { DetailHeader, InlineSwitch, SettingsField, SkTextInput } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import { composeSkillDocument, parseSkillDocument } from "./skill-document";

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

/**
 * 编辑新建或已安装 Skill 的完整 SKILL.md。
 *
 * 编辑区展示完整 SKILL.md，包括开头的名称与描述前言。
 * 正文区复用两态 Markdown 编辑器，可在源码与可编辑预览之间切换。
 *
 * @param props 当前条目、文档内容、保存状态与更新回调
 * @returns Skill 文档编辑区
 */
export function SkillEditor(props: SkillEditorProps) {
  const { t } = useI18n();
  const { theme } = useTheme();
  const { skill, content, directoryName, creating, dirty, saving, error } = props;
  const [mode, setMode] = useMarkdownMode();
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
    <section className="grid min-w-0 gap-4">
      <DetailHeader
        title={creating ? t("New Skill", "新增 Skill") : (skill?.name ?? t("Skill manager", "Skill 管理"))}
        subtitle={creating
          ? t("Create a global Skill directory with a complete SKILL.md document.", "在全局目录新增 Skill，并写入完整 SKILL.md。")
          : (skill ? skill.path : t("Select a Skill to inspect or edit its document.", "选择 Skill 后查看或编辑文档。"))}
        actions={creating || skill ? (
          <>
            {skill && !creating && (
              <InlineSwitch label={t("Enabled", "已启用")} checked={skill.enabled} onChange={props.onEnabledChange} />
            )}
            <Button variant="primary" className="skill-save-button" disabled={!dirty || saving} onClick={props.onSave}>
              <Save size={14} />
              {saving ? t("Saving", "正在保存") : t("Save", "保存")}
            </Button>
          </>
        ) : undefined}
      />

      {error && <div className="settings-inline-error">{error}</div>}
      {!creating && !skill ? (
        <div className="settings-empty">
          <BookOpen size={20} />
          <p>{t("Scan or select a Skill to manage it.", "扫描或选择一个 Skill 进行管理。")}</p>
        </div>
      ) : (
        <>
          {creating && (
            <SettingsField label={t("Directory name", "目录名称")}><SkTextInput value={directoryName} onChange={(name) => { props.onDirectoryNameChange(name); updateDocument({ name }); }} placeholder="code-review" /></SettingsField>
          )}
          <section className="skill-document-workspace">
            <div className="skill-doc-toolbar">
              <div>
                <h3>SKILL.md</h3>
                <span>{t("Full document, including the name and description frontmatter", "完整文档，含名称与描述前言")}</span>
              </div>
              <MarkdownModeToggle mode={mode} onChange={setMode} t={t} />
            </div>

            <div className="skill-content-editor" aria-label={t("Skill document", "Skill 文档")}>
              <MarkdownEditor
                value={content}
                onChange={props.onContentChange}
                mode={mode}
                dark={isDarkTheme(theme)}
                onModeChange={setMode}
              />
            </div>
          </section>
        </>
      )}
    </section>
  );
}
