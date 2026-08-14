import { useState } from "react";
import { Plus } from "lucide-react";
import type { MemoryType, MemoryWriteRequest } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";

type MemoryComposeFormProps = {
  pending: boolean;
  onSubmit: (request: MemoryWriteRequest) => void;
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
 * @param props 提交状态与回调
 * @returns 新建表单
 */
export function MemoryComposeForm({ pending, onSubmit }: MemoryComposeFormProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [content, setContent] = useState("");
  const [memoryType, setMemoryType] = useState<MemoryType>("feedback");
  const [global, setGlobal] = useState(false);

  const ready = name.trim().length > 0 && description.trim().length > 0 && content.trim().length > 0;

  /** 提交并清空表单。 */
  const submit = () => {
    if (!ready) return;
    onSubmit({
      name: name.trim(),
      description: description.trim(),
      content: content.trim(),
      memory_type: memoryType,
      global
    });
    setName("");
    setDescription("");
    setContent("");
  };

  return (
    <div className="memory-compose">
      <div className="memory-compose-row">
        <label className="memory-compose-field">
          <span>{t("Identifier", "标识")}</span>
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t("kebab-case, also the file name", "短横线分隔，同时是文件名")}
          />
        </label>
        <label className="memory-compose-field">
          <span>{t("Summary", "摘要")}</span>
          <input
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder={t("One line shown in the index", "索引里显示的一行")}
          />
        </label>
      </div>
      <div className="memory-compose-row">
        <label className="memory-compose-field">
          <span>{t("Type", "类型")}</span>
          <Select
            value={memoryType}
            options={TYPE_HINTS.map((hint) => ({
              value: hint.value,
              label: t(hint.en, hint.zh),
              description: t(hint.hintEn, hint.hintZh)
            }))}
            ariaLabel={t("Choose memory type", "选择记忆类型")}
            onChange={(value) => setMemoryType(value as MemoryType)}
          />
        </label>
        <label className="memory-compose-scope">
          <input type="checkbox" checked={global} onChange={(event) => setGlobal(event.target.checked)} />
          <span>
            <strong>{t("Global", "全局")}</strong>
            <small>{t("Applies in every workspace", "在所有工作区生效")}</small>
          </span>
        </label>
      </div>
      <textarea
        value={content}
        onChange={(event) => setContent(event.target.value)}
        placeholder={t(
          "The fact itself. For feedback and project, add Why: and How to apply: lines.",
          "事实本身。工作方式与项目约束需补 Why: 与 How to apply: 两行。"
        )}
        rows={4}
      />
      <button type="button" onClick={submit} disabled={!ready || pending}>
        <Plus size={14} /> {pending ? t("Saving", "保存中") : t("Save memory", "保存记忆")}
      </button>
    </div>
  );
}
