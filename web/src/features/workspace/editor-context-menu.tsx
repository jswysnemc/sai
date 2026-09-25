import { Copy, Save, WrapText } from "lucide-react";
import { useEffect, useRef } from "react";
import { useI18n } from "../i18n/use-i18n";
import { useClampedMenuPosition } from "./menu-position";

type EditorContextMenuProps = {
  x: number;
  y: number;
  path: string;
  /** 为 null 时不提供换行项（如图片） */
  wordWrap: boolean | null;
  savable: boolean;
  canSave: boolean;
  onToggleWordWrap: () => void;
  onSave: () => void;
  onClose: () => void;
};

/** 渲染编辑器标题栏的右键菜单。 */
export function EditorContextMenu(props: EditorContextMenuProps) {
  const { t } = useI18n();
  const ref = useRef<HTMLDivElement>(null);
  const position = useClampedMenuPosition(props.x, props.y, ref);
  useEffect(() => {
    const close = (event: PointerEvent) => { if (!ref.current?.contains(event.target as Node)) props.onClose(); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") props.onClose(); };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", escape);
    return () => { document.removeEventListener("pointerdown", close); document.removeEventListener("keydown", escape); };
  }, [props]);

  const copyPath = () => {
    props.onClose();
    void navigator.clipboard.writeText(props.path);
  };

  return (
    <div ref={ref} className="editor-context-menu" style={position} role="menu">
      <button type="button" role="menuitem" onClick={copyPath}><Copy size={13} />{t("Copy path", "复制路径")}</button>
      {props.wordWrap !== null && (
        <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onToggleWordWrap(); }}>
          <WrapText size={13} />{props.wordWrap ? t("Disable word wrap", "关闭自动换行") : t("Enable word wrap", "开启自动换行")}
        </button>
      )}
      {props.savable && (
        <button type="button" role="menuitem" onClick={() => { props.onClose(); props.onSave(); }} disabled={!props.canSave}>
          <Save size={13} />{t("Save", "保存")}
        </button>
      )}
    </div>
  );
}
