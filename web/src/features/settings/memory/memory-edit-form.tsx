import { useState } from "react";
import type { api } from "../../../api/client";
import type { MemoryType } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Check, X } from "../../../shared/ui/icons";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SkTextInput, SkTextArea } from "../kit";
import { missingRationaleMarkers } from "./memory-filter";

type MemoryEditFormProps = {
  name: string;
  description: string;
  content: string;
  /** 索引里的提示行；保存时原样带回，空值后端会沿用摘要 */
  hook: string;
  memoryType: MemoryType;
  global: boolean;
  workspace?: string;
  pending: boolean;
  onCancel: () => void;
  onSave: (request: Parameters<typeof api.memory.remember>[0]) => void;
};

/**
 * 就地编辑表单：改摘要、索引提示与正文。
 *
 * 标识、类型与作用域不在编辑范围：标识是文件名与链接目标，改名等于
 * 换一条记忆；类型与作用域选错，删了重建比改更不容易出半吊子状态。
 *
 * @param props 原值与操作回调
 * @returns 编辑表单
 */
export function MemoryEditForm({
  name,
  description,
  content,
  hook,
  memoryType,
  global,
  workspace,
  pending,
  onCancel,
  onSave
}: MemoryEditFormProps) {
  const { t } = useI18n();
  const [nextDescription, setNextDescription] = useState(description);
  const [nextHook, setNextHook] = useState(hook);
  const [nextContent, setNextContent] = useState(content);
  const missing = missingRationaleMarkers(memoryType, nextContent);

  return (
    <div className="memory-edit">
      <FieldGrid>
        <SettingsField label={t("Summary", "摘要")}>
          <SkTextInput value={nextDescription} onChange={setNextDescription} disabled={pending} />
        </SettingsField>
        <SettingsField label={t("Index hook", "索引提示")}>
          <SkTextInput value={nextHook} onChange={setNextHook} disabled={pending} placeholder={t("Optional; defaults to the summary", "可选；留空沿用摘要")} />
        </SettingsField>
        <SettingsField label={t("Content", "正文")} span="full">
          <SkTextArea value={nextContent} onChange={setNextContent} rows={6} disabled={pending} />
        </SettingsField>
      </FieldGrid>
      {missing.length > 0 && (
        <div className="memory-rationale-hint">
          {t(
            `Missing ${missing.join(" and ")} — without them a later turn cannot judge whether this still applies.`,
            `缺 ${missing.join(" 与 ")}——缺了理由，下一轮无法判断这条在新情境下还适不适用。`
          )}
        </div>
      )}
      <div className="memory-edit-actions">
        <Button type="button" variant="secondary" onClick={onCancel} disabled={pending}>
          <X size={14} /> {t("Cancel", "取消")}
        </Button>
        <Button
          type="button"
          disabled={pending || nextContent.trim().length === 0 || nextDescription.trim().length === 0}
          onClick={() =>
            onSave({
              name,
              description: nextDescription.trim(),
              content: nextContent,
              memory_type: memoryType,
              global,
              hook: nextHook.trim(),
              workspace
            })
          }
        >
          <Check size={14} /> {pending ? t("Saving", "保存中") : t("Save changes", "保存修改")}
        </Button>
      </div>
      {nextDescription.trim().length === 0 && (
        <div className="memory-rationale-hint">
          {t("A summary is required.", "摘要不能为空。")}
        </div>
      )}
    </div>
  );
}
