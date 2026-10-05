import { Copy, Trash2, X } from "../../shared/ui/icons";
import { useI18n } from "../i18n/use-i18n";

type FileTreeSelectionBarProps = {
  count: number;
  onCopyPaths: () => void;
  onDelete: () => void;
  onClear: () => void;
};

/**
 * 【工作区】【文件树多选】多选两项以上时显示的紧凑操作条。
 *
 * @param props 已选数量与批量操作回调
 * @returns 选择计数、复制路径、删除与退出按钮
 */
export function FileTreeSelectionBar({ count, onCopyPaths, onDelete, onClear }: FileTreeSelectionBarProps) {
  const { t } = useI18n();
  return (
    <div className="file-tree-selection-bar" role="toolbar" aria-label={t("Selected files", "已选文件")}>
      <span>{t(`${count} selected`, `已选 ${count} 项`)}</span>
      <button type="button" onClick={onCopyPaths} aria-label={t("Copy relative paths", "复制相对路径")} title={t("Copy relative paths", "复制相对路径")}><Copy size={12} /></button>
      <button type="button" className="danger" onClick={onDelete} aria-label={t("Delete selected", "删除所选")} title={t("Delete selected", "删除所选")}><Trash2 size={12} /></button>
      <button type="button" onClick={onClear} aria-label={t("Clear selection", "取消选择")} title={t("Clear selection (Esc)", "取消选择（Esc）")}><X size={12} /></button>
    </div>
  );
}
