import { Plus, Tag, Trash2 } from "lucide-react";
import { useState } from "react";
import type { GitTag } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { RunGitOperation } from "../types";

type TagSectionProps = {
  tags: GitTag[];
  busy: boolean;
  runOperation: RunGitOperation;
};

/**
 * 渲染标签创建、列表和删除操作。
 *
 * @param props 标签数据、忙碌状态和 Git 操作回调
 * @returns 标签资源分区
 */
export function TagSection(props: TagSectionProps) {
  const { t } = useI18n();
  const [tagName, setTagName] = useState("");

  /**
   * 创建 HEAD 标签并在成功后清空输入。
   *
   * @returns 无返回值
   */
  const createTag = async () => {
    const tag = tagName.trim();
    if (!tag) return;
    const result = await props.runOperation("tag_create", { tag });
    if (result?.ok) setTagName("");
  };

  return (
    <>
      <span>{t("Tags", "标签")}</span>
      <div className="git-resource-create compact">
        <Tag size={12} />
        <input value={tagName} onChange={(event) => setTagName(event.target.value)} placeholder={t("Tag name", "标签名称")} spellCheck={false} />
        <Button disabled={props.busy || !tagName.trim()} onClick={() => void createTag()} title={t("Create tag at HEAD", "在 HEAD 创建标签")}><Plus size={11} /></Button>
      </div>
      {props.tags.slice(0, 8).map((tag) => (
        <div className="git-resource-row" key={tag.name}>
          <span title={tag.subject}><strong>{tag.name}</strong><small>{tag.sha.slice(0, 7)}</small></span>
          <div>
            <Button disabled={props.busy} title={t("Delete tag", "删除标签")} onClick={() => void props.runOperation("tag_delete", {
              tag: tag.name,
              confirmTitle: t("Delete tag?", "删除标签？"),
              confirmDescription: tag.name
            })}><Trash2 size={11} /></Button>
          </div>
        </div>
      ))}
    </>
  );
}
