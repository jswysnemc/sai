import { Copy, FilePlus2, FolderOpen, FolderPlus, Pencil, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import { useI18n } from "../i18n/use-i18n";

type FileTreeContextMenuProps = {
  x: number;
  y: number;
  path: string;
  directory: boolean;
  onOpen: () => void;
  onCreate: (kind: "file" | "directory") => void;
  onRename: () => void;
  onDelete: () => void;
  onCopyPath: () => void;
  onClose: () => void;
};

/** 渲染工作区文件树的通用右键菜单。 */
export function FileTreeContextMenu(props: FileTreeContextMenuProps) {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const close = (event: PointerEvent) => { if (!ref.current?.contains(event.target as Node)) props.onClose(); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") props.onClose(); };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", escape); };
  }, [props]);
  return (
    <div ref={ref} className="file-tree-context-menu" style={{ left: props.x, top: props.y }} role="menu">
      {!props.directory && <button type="button" onClick={() => { props.onClose(); props.onOpen(); }}><FolderOpen size={13} />{t("Open", "打开")}</button>}
      <button type="button" onClick={() => { props.onClose(); props.onCopyPath(); }}><Copy size={13} />{t("Copy path", "复制路径")}</button>
      {props.directory && <>
        <button type="button" onClick={() => { props.onClose(); props.onCreate("file"); }}><FilePlus2 size={13} />{t("New file", "新建文件")}</button>
        <button type="button" onClick={() => { props.onClose(); props.onCreate("directory"); }}><FolderPlus size={13} />{t("New folder", "新建文件夹")}</button>
      </>}
      <button type="button" onClick={() => { props.onClose(); props.onRename(); }}><Pencil size={13} />{t("Rename", "重命名")}</button>
      <button type="button" className="danger" onClick={() => { props.onClose(); props.onDelete(); }}><Trash2 size={13} />{t("Delete", "删除")}</button>
    </div>
  );
}
