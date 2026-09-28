import { useState } from "react";
import { Plus, X } from "../../../shared/ui/icons";
import type { MemoryType, MemoryWriteRequest, MemoryWriteResult } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { ChoicePills, FieldGrid, SettingsField, SettingsPanel, SkTextArea, SkTextInput, SwitchField } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import { missingRationaleMarkers } from "./memory-filter";

type MemoryComposeFormProps = {
  pending: boolean;
  workspace?: string;
  /** 折叠表单回调：录入是低频动作，表单默认收起 */
  onCollapse: () => void;
  /** 返回写入结果；失败时表单内容保留，用户输入不能丢 */
  onSubmit: (request: MemoryWriteRequest) => Promise<MemoryWriteResult | null>;
};

/** 条目类型的可选项与说明。 */
const TYPE_HINTS: Array<{ value: MemoryType; en: string; zh: string; hintEn: string; hintZh: string }> = [
  {
    value: "feedback",
    en: "Feedback",
    zh: "工作方式要求",
    hintEn: "How the assistant should work; state the reason",
    hintZh: "要求助手怎么做，需写明理由"
  },
  { value: "user", en: "User", zh: "关于用户", hintEn: "Role, expertise, standing preferences", hintZh: "角色、专长、长期偏好" },
  {
    value: "project",
    en: "Project",
    zh: "项目约束",
    hintEn: "Ongoing work not derivable from the code",
    hintZh: "无法从代码看出的进行中工作与约束"
  },
  { value: "reference", en: "Reference", zh: "外部资源", hintEn: "URLs, boards, tickets", hintZh: "网址、看板、工单" }
];

/**
 * 新建一条记忆的表单。
 *
 * 标识、摘要、类型、作用域都要显式填：文件式记忆靠标识定位与关联，
 * 让它自动生成会让同一件事被反复记成互不相干的多条。
 *
 * @param props 提交状态、工作区标识与回调
 * @returns 新建表单
 */
export function MemoryComposeForm({ pending, workspace, onCollapse, onSubmit }: MemoryComposeFormProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [hook, setHook] = useState("");
  const [content, setContent] = useState("");
  const [memoryType, setMemoryType] = useState<MemoryType>("feedback");
  const [global, setGlobal] = useState(false);

  const missing = missingRationaleMarkers(memoryType, content);
  const ready = name.trim().length > 0 && description.trim().length > 0 && content.trim().length > 0;

  /** 提交并清空表单；失败时保留输入。 */
  const submit = async () => {
    if (!ready) return;
    const result = await onSubmit({
      name: name.trim(),
      description: description.trim(),
      content: content.trim(),
      memory_type: memoryType,
      global,
      hook: hook.trim(),
      workspace
    });
    if (!result) return;
    setName("");
    setDescription("");
    setHook("");
    setContent("");
  };

  return <SettingsPanel title={t("New memory", "新建记忆")} actions={<Button variant="secondary" onClick={onCollapse} disabled={pending}><X size={14} />{t("Collapse", "收起")}</Button>}>
    <FieldGrid>
      <SettingsField label={t("Identifier", "标识")}><SkTextInput value={name} onChange={setName} placeholder={t("kebab-case, also the file name", "短横线分隔，同时是文件名")} /></SettingsField>
      <SettingsField label={t("Summary", "摘要")}><SkTextInput value={description} onChange={setDescription} /></SettingsField>
      <SettingsField label={t("Type", "类型")} span="full"><ChoicePills value={memoryType} onChange={setMemoryType} options={TYPE_HINTS.map((hint) => ({ value: hint.value, label: t(hint.en, hint.zh) }))} /></SettingsField>
      <SettingsField label={t("Index hook", "索引提示")} hint={t("Optional; defaults to the summary.", "可选；留空沿用摘要。") }><SkTextInput value={hook} onChange={setHook} /></SettingsField>
      <SwitchField label={t("Global", "全局")} hint={t("Applies in every workspace.", "在所有工作区生效。") } checked={global} onChange={setGlobal} />
      <SettingsField label={t("Content", "正文")} span="full" hint={missing.length ? t(`Add ${missing.join(" and ")} to explain when this applies.`, `补充 ${missing.join(" 与 ")}，说明适用条件。`) : undefined}><SkTextArea value={content} onChange={setContent} rows={4} /></SettingsField>
    </FieldGrid>
    <div><Button variant="primary" onClick={() => void submit()} disabled={!ready || pending}><Plus size={14} />{pending ? t("Saving", "保存中") : t("Save memory", "保存记忆")}</Button></div>
  </SettingsPanel>;
}

/**
 * 写入结果的内联反馈：更新了同名条目或后端要求补写理由时提示。
 *
 * @param result 写入接口的响应
 * @returns 提示元素；无话可说时为空
 */
export function MemoryWriteFeedback({ result }: { result: MemoryWriteResult | null }) {
  const { t } = useI18n();
  if (!result) return null;
  return (
    <div className="memory-write-feedback">
      {result.updated &&
        t("Updated the existing memory with the same identifier.", "已更新同名记忆。")}
      {result.updated && result.note ? " " : ""}
      {result.note}
    </div>
  );
}
