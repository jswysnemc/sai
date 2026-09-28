import { useMemo } from "react";
import type { PromptTemplateConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { segmentLinePair } from "../../chat/tool-renderers/diff/inline-diff";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsPanel } from "../kit";

type PromptResetPreviewProps = {
  name: string;
  current: PromptTemplateConfig;
  defaults: PromptTemplateConfig;
  onClose: () => void;
  onConfirm: () => void;
};

/**
 * 【Web 设置】【提示词恢复】对照当前提示词与默认值，确认后才修改草稿。
 * @param props 模板名称、当前文本、默认文本及关闭和确认回调
 * @returns 带差异高亮的确认弹窗
 */
export function PromptResetPreview({ name, current, defaults, onClose, onConfirm }: PromptResetPreviewProps) {
  const { t } = useI18n();
  const fields = useMemo(() => (["system", "user"] as const).map((key) => ({
    key, changed: current[key] !== defaults[key], ...segmentLinePair(current[key], defaults[key])
  })), [current, defaults]);
  return (
    <Modal open size="large" title={t(`Restore ${name}`, `恢复${name}`)} description={t("Review the changes before restoring. The result remains a draft until saved.", "先核对差异再恢复，恢复结果仍需通过顶栏保存。")} onClose={onClose}
      footer={<><Button onClick={onClose}>{t("Cancel", "取消")}</Button><Button variant="primary" onClick={onConfirm}>{t("Restore default", "恢复默认")}</Button></>}>
      {fields.map(({ key, before, after, changed }) => (
        <SettingsPanel key={key} title={key === "system" ? t("System instruction", "系统指令") : t("Input template", "输入模板")}>
          {!changed ? <p className="sk-field-hint">{t("Already matches the default.", "已与默认值一致。")}</p> : (
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              {[{ label: t("Current", "当前内容"), segments: before, className: "is-removed" }, { label: t("Default", "默认内容"), segments: after, className: "is-added" }].map(({ label, segments, className }) => (
                <div key={className} className="min-w-0">
                  <p className="sk-field-hint">{label}</p>
                  <pre className="prompt-reset-text">{segments.map((segment, index) => <span key={index} className={segment.changed ? className : undefined}>{segment.text}</span>)}</pre>
                </div>
              ))}
            </div>
          )}
        </SettingsPanel>
      ))}
    </Modal>
  );
}
