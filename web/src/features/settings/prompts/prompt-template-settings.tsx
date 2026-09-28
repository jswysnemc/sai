import { useRef, useState } from "react";
import { PromptResetPreview } from "./prompt-reset-preview";
import { RotateCcw } from "../../../shared/ui/icons";
import type { PromptTemplateConfig, PromptTemplatesConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkTextArea } from "../kit";
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
  const [resetId, setResetId] = useState<PromptTemplateId | null>(null);
  const inputs = useRef(new Map<string, HTMLTextAreaElement>());
  const activeFields = useRef(new Map<PromptTemplateId, keyof PromptTemplateConfig>());

  /**
   * 【提示词】【变量插入】在最近编辑的位置插入缺少的变量，已有变量则定位并选中。
   * @param id 模板标识
   * @param name 变量名
   * @returns 无返回值
   */
  const insertVariable = (id: PromptTemplateId, name: string) => {
    const token = `{{${name}}}`;
    const template = templates[id];
    const existingField = (["system", "user"] as const).find((field) => template[field].includes(token));
    const field = existingField ?? activeFields.current.get(id) ?? "user";
    const input = inputs.current.get(`${id}.${field}`);
    if (!input) return;
    if (existingField) {
      const start = template[field].indexOf(token);
      input.focus();
      input.setSelectionRange(start, start + token.length);
      return;
    }
    const start = input.selectionStart;
    const end = input.selectionEnd;
    updateTemplate(id, field, template[field].slice(0, start) + token + template[field].slice(end));
    requestAnimationFrame(() => {
      input.focus();
      input.setSelectionRange(start + token.length, start + token.length);
    });
  };

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
    <>
      {PROMPT_TEMPLATE_DEFINITIONS.map((definition) => (
        <SettingsPanel
          key={definition.id}
          title={t(definition.labelEn, definition.labelZh)}
          description={t(definition.descriptionEn, definition.descriptionZh)}
          actions={(
            <Button
              className="prompt-template-reset"
              onClick={() => setResetId(definition.id)}
              title={t("Restore this prompt", "恢复该提示词")}
            >
              <RotateCcw size={14} />
              {t("Restore default", "恢复默认")}
            </Button>
          )}
        >
          <FieldGrid>
            <SettingsField label={t("System instruction", "系统指令")} anchor={`prompts.${definition.id}.system`} configKey={`prompt.templates.${definition.id}.system`} hint={t("Defines the task, output format, and constraints.", "定义任务、输出格式和约束。")}>
              <SkTextArea mono rows={6}
                ref={(input) => { if (input) inputs.current.set(`${definition.id}.system`, input); else inputs.current.delete(`${definition.id}.system`); }}
                onFocus={() => activeFields.current.set(definition.id, "system")}
                value={templates[definition.id].system}
                onChange={(value) => updateTemplate(definition.id, "system", value)}
                spellCheck={false}
              />
            </SettingsField>
            <SettingsField label={t("Input template", "输入模板")} anchor={`prompts.${definition.id}.user`} configKey={`prompt.templates.${definition.id}.user`} hint={t("Required variables must appear exactly once.", "必要变量必须各保留一次。")}>
              <SkTextArea mono rows={6}
                ref={(input) => { if (input) inputs.current.set(`${definition.id}.user`, input); else inputs.current.delete(`${definition.id}.user`); }}
                onFocus={() => activeFields.current.set(definition.id, "user")}
                value={templates[definition.id].user}
                onChange={(value) => updateTemplate(definition.id, "user", value)}
                spellCheck={false}
              />
            </SettingsField>
          </FieldGrid>
          <div className="prompt-template-variables" aria-label={t("Available variables", "可用变量")}>
            <span>{t("Required variables · insert missing or locate existing", "必要变量 · 点击插入缺失变量或定位已有变量")}</span>
            <div>
              {definition.variables.map((variable) => (
                <Button size="small" key={variable.name} title={t(variable.descriptionEn, variable.descriptionZh)} onClick={() => insertVariable(definition.id, variable.name)}>
                  {`{{${variable.name}}}`}
                </Button>
              ))}
            </div>
          </div>
        </SettingsPanel>
      ))}
      {resetId && <PromptResetPreview
        name={t(PROMPT_TEMPLATE_DEFINITIONS.find((item) => item.id === resetId)!.labelEn, PROMPT_TEMPLATE_DEFINITIONS.find((item) => item.id === resetId)!.labelZh)}
        current={templates[resetId]} defaults={DEFAULT_PROMPT_TEMPLATES[resetId]} onClose={() => setResetId(null)}
        onConfirm={() => { resetTemplate(resetId); setResetId(null); }}
      />}
    </>
  );
}
