import { useEffect, useRef } from "react";
import { useI18n } from "../i18n/use-i18n";

type FileTreeContextMenuProps = {
  x: number;
  y: number;
  path: string;
  directory: boolean;
  canPaste: boolean;
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
  const rooted = props.path !== "";
  const pasteDirectory = props.directory || !rooted;
  useEffect(() => {
    const close = (event: PointerEvent) => { if (!ref.current?.contains(event.target as Node)) props.onClose(); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") props.onClose(); };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", escape); };
  }, [props]);
  return (
    <div ref={ref} className="file-tree-context-menu" style={{ left: props.x, top: props.y }} role="menu">
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
