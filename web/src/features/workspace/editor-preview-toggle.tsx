import { Eye, Pencil } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";

type EditorPreviewToggleProps = {
  preview: boolean;
  onChange: (preview: boolean) => void;
};

/**
 * 渲染 Markdown 编辑与预览模式切换控件。
 *
 * @param props 当前模式与模式变更回调
 * @returns 编辑器头部模式切换控件
 */
export function EditorPreviewToggle({ preview, onChange }: EditorPreviewToggleProps) {
  const { t } = useI18n();
  return (
    <div className="editor-preview-toggle" role="group" aria-label={t("Markdown display mode", "Markdown 显示模式")}>
      <button type="button" className={!preview ? "active" : ""} onClick={() => onChange(false)} aria-pressed={!preview}>
        <Pencil size={13} />
        {t("Edit", "编辑")}
      </button>
      <button type="button" className={preview ? "active" : ""} onClick={() => onChange(true)} aria-pressed={preview}>
        <Eye size={13} />
        {t("Preview", "预览")}
      </button>
    </div>
  );
}
