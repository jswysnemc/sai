import { useEffect, useRef } from "react";
import { useI18n } from "../i18n/use-i18n";
import { useClampedMenuPosition } from "./menu-position";

type FileTreeContextMenuProps = {
  x: number;
  y: number;
  path: string;
  directory: boolean;
  canPaste: boolean;
  /** 右键时的已选条目数；大于 1 时只保留批量可用的操作 */
  selectedCount?: number;
  onOpenContaining: () => void;
  onCreate: (kind: "file" | "directory") => void;
  onCopyPath: () => void;
  onCopyRelativePath: () => void;
  onCut: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onRename: () => void;
  onDelete: () => void;
  onClose: () => void;
};

/** 渲染与资源管理器一致的文件树右键菜单。 */
export function FileTreeContextMenu(props: FileTreeContextMenuProps) {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  const position = useClampedMenuPosition(props.x, props.y, ref);
  const rooted = props.path !== "";
  const pasteDirectory = props.directory || !rooted;
  const count = props.selectedCount ?? 1;
  useEffect(() => {
    const close = (event: PointerEvent) => { if (!ref.current?.contains(event.target as Node)) props.onClose(); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") props.onClose(); };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", escape); };
  }, [props]);
  // 1. 多选：复制路径与删除作用于全部已选条目，单项操作隐藏
  if (rooted && count > 1) {
    return (
      <div ref={ref} className="file-tree-context-menu" style={position} role="menu">
        <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCopyPath(); }}>{t(`Copy ${count} Paths`, `复制 ${count} 个路径`)}</button>
        <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCopyRelativePath(); }}>{t(`Copy ${count} Relative Paths`, `复制 ${count} 个相对路径`)}</button>
        <div className="file-tree-context-separator" role="separator" />
        <button type="button" role="menuitem" className="danger" onClick={() => { props.onClose(); props.onDelete(); }}>{t(`Delete ${count} Items`, `删除 ${count} 项`)}</button>
      </div>
    );
  }
  return (
    <div ref={ref} className="file-tree-context-menu" style={position} role="menu">
      {rooted && <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onOpenContaining(); }}>{t("Open Containing Folder", "打开所在文件夹")}</button>}
      <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCreate("file"); }}>{t("New File", "新建文件")}</button>
      <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCreate("directory"); }}>{t("New Folder", "新建文件夹")}</button>
      {rooted && <div className="file-tree-context-separator" role="separator" />}
      {rooted && <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCopyPath(); }}>{t("Copy Path", "复制路径")}</button>}
      {rooted && <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCopyRelativePath(); }}>{t("Copy Relative Path", "复制相对路径")}</button>}
      {rooted && <div className="file-tree-context-separator" role="separator" />}
      {rooted && <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCut(); }}>{t("Cut", "剪切")}</button>}
      {rooted && <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onCopy(); }}>{t("Copy", "复制")}</button>}
      <button type="button" role="menuitem" disabled={!props.canPaste || !pasteDirectory} onClick={() => { props.onClose(); props.onPaste(); }}>{t("Paste", "粘贴")}</button>
      {rooted && <div className="file-tree-context-separator" role="separator" />}
      {rooted && <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onRename(); }}>{t("Rename", "重命名")}</button>}
      {rooted && <button type="button" role="menuitem" className="danger" onClick={() => { props.onClose(); props.onDelete(); }}>{t("Delete", "删除")}</button>}
    </div>
  );
}
