import { FileTypeIcon } from "../../../shared/ui/file-icon";
import type { MouseEvent } from "react";

type ToolFileReferenceProps = {
  path: string;
  className?: string;
  icon?: boolean;
};

/**
 * 渲染可在工作区编辑器中打开的文件路径。
 *
 * @param props path 为文件路径，className 为附加样式，icon 控制文件图标
 * @returns 文件路径按钮
 */
export function ToolFileReference({ path, className = "", icon = true }: ToolFileReferenceProps) {
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
      {icon && <FileTypeIcon name={path} size={13} />}
      <button type="button" onClick={openFile} title="在编辑器中打开">
        {path}
      </button>
    </span>
  );
}
