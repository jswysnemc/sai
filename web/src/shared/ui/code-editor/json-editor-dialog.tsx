import type { ReactNode } from "react";
import { Modal } from "../dialog/modal";

/**
 * 展示覆盖可用视口的 JSON 编辑窗口，复用统一焦点与关闭行为。
 * @param props 窗口名称、编辑器内容和关闭回调
 * @returns 全屏编辑窗口
 */
export function JsonEditorDialog({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  return <Modal open title={title} onClose={onClose} flush size="large" className="json-editor-dialog">{children}</Modal>;
}
