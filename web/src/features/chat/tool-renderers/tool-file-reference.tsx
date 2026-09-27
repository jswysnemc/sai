import { FileTypeIcon } from "../../../shared/ui/file-icon";
import { type MouseEvent } from "react";
import { formatDisplayPath } from "../../workspace/workspace-path-utils";

type ToolFileReferenceProps = {
  path: string;
  /** 展示文案；缺省使用 path */
  label?: string;
  /** 当前工作区根目录；提供后默认标签显示相对路径 */
  workspacePath?: string;
  className?: string;
  icon?: boolean;
};

/**
 * 渲染可在工作区编辑器中打开的文件路径。
 *
 * @param props path 为打开路径，label 为展示文案，className 为附加样式，icon 控制文件图标
 * @returns 文件路径按钮
 */
export function ToolFileReference({ path, label, workspacePath = "", className = "", icon = true }: ToolFileReferenceProps) {
  const displayLabel = label || formatDisplayPath(path, workspacePath) || formatDisplayPath(path, "");
  const { directory, name } = splitReferenceLabel(displayLabel || path);

  /**
   * 派发工作区统一文件打开事件。
   *
   * @param event 文件路径按钮点击事件
   * @returns 无返回值
   */
  const openFile = (event: MouseEvent<HTMLButtonElement>) => {
    // 1. 阻止工具卡头部同时执行展开操作
    event.stopPropagation();
    // 2. 通知工作区编辑器打开目标文件
    window.dispatchEvent(new CustomEvent("sai:open-file", { detail: { path } }));
  };

  return (
    <span className={`tool-file-reference ${className}`.trim()}>
      <button type="button" onClick={openFile} title={displayLabel || path}>
        {icon && <FileTypeIcon name={path} size={14} />}
        <span className="tool-file-reference-label">
          {directory ? <span className="tool-file-reference-dir">{directory}</span> : null}
          <span className="tool-file-reference-name">{name}</span>
        </span>
      </button>
    </span>
  );
}

/**
 * 把路径拆成目录和带斜杠的文件名，窄宽度时先省略目录、留下文件名。
 *
 * @param label 展示用路径
 * @returns 目录（不含末级斜杠）和文件名（含前导斜杠）
 */
function splitReferenceLabel(label: string): { directory: string; name: string } {
  const cut = Math.max(label.lastIndexOf("/"), label.lastIndexOf("\\"));
  if (cut <= 0) return { directory: "", name: label };
  return { directory: label.slice(0, cut), name: label.slice(cut) };
}
