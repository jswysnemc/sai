import { RotateCcw } from "lucide-react";
import type { PromptTemplateConfig, PromptTemplatesConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { EditorHeader, SettingsGroup } from "../editor-layout";
import {
  DEFAULT_PROMPT_TEMPLATES,
  PROMPT_TEMPLATE_DEFINITIONS,
  type PromptTemplateId
} from "./prompt-template-catalog";
import "./prompt-template-settings.css";

type PromptTemplateSettingsProps = {
  templates: PromptTemplatesConfig;
  onChange: (templates: PromptTemplatesConfig) => void;
};

/**
 * 渲染 Sai 内部模型任务的提示词编辑区域。
 *
 * @param props 当前模板和更新回调
 * @returns 提示词设置区域
 */
export function PromptTemplateSettings({ templates, onChange }: PromptTemplateSettingsProps) {
  const { t } = useI18n();

  /**
   * 更新指定任务的一段提示词。
   *
   * @param id 模板任务标识
   * @param field 系统提示词或用户输入模板
   * @param value 新文本
   * @returns 无返回值
   */
  const updateTemplate = (id: PromptTemplateId, field: keyof PromptTemplateConfig, value: string) => {
    onChange({
      ...templates,
      [id]: { ...templates[id], [field]: value }
    });
  };

  /**
   * 将指定任务恢复为内置默认模板。
   *
   * @param id 模板任务标识
   * @returns 无返回值
   */
  const resetTemplate = (id: PromptTemplateId) => {
    onChange({
      ...templates,
      [id]: { ...DEFAULT_PROMPT_TEMPLATES[id] }
    });
  };

  return (
    <section className="settings-editor prompt-template-settings">
      <EditorHeader
        kicker={t("Prompts", "提示词")}
        title={t("Internal task prompts", "内部任务提示词")}
        description={t(
          "Edit the prompts used for commit messages, session titles, and context compaction. Required variables must remain exactly once.",
          "编辑提交说明、会话标题和上下文压缩提示词；必要变量必须各保留一次。"
        )}
      />
      {PROMPT_TEMPLATE_DEFINITIONS.map((definition) => (
        <SettingsGroup
          key={definition.id}
          title={t(definition.labelEn, definition.labelZh)}
          description={t(definition.descriptionEn, definition.descriptionZh)}
          actions={(
            <Button
              className="prompt-template-reset"
              onClick={() => resetTemplate(definition.id)}
              title={t("Restore this prompt", "恢复该提示词")}
            >
              <RotateCcw size={13} />
              {t("Restore default", "恢复默认")}
            </Button>
          )}
        >
          <div className="prompt-template-grid">
            <label className="settings-field prompt-template-field">
              <span>{t("System instruction", "系统指令")}</span>
              <textarea
                value={templates[definition.id].system}
                onChange={(event) => updateTemplate(definition.id, "system", event.target.value)}
                spellCheck={false}
              />
              <small>{t("Defines the task, output format, and constraints.", "定义任务、输出格式和约束。")}</small>
            </label>
            <label className="settings-field prompt-template-field">
              <span>{t("Input template", "输入模板")}</span>
              <textarea
                value={templates[definition.id].user}
                onChange={(event) => updateTemplate(definition.id, "user", event.target.value)}
                spellCheck={false}
              />
              <small>{t("Variables are replaced immediately before the model request.", "变量会在模型请求前完成替换。")}</small>
            </label>
          </div>
          <div className="prompt-template-variables" aria-label={t("Available variables", "可用变量")}>
            <span>{t("Required variables", "必要变量")}</span>
            <div>
              {definition.variables.map((variable) => (
                <code key={variable.name} title={t(variable.descriptionEn, variable.descriptionZh)}>
                  {`{{${variable.name}}}`}
                </code>
              ))}
            </div>
          </div>
        </SettingsGroup>
      ))}
    </section>
  );
}
