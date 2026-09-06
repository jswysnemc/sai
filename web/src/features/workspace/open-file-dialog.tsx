import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useRef, useState } from "react";
import { api } from "../../api/client";
import { ApiError, toDisplayError } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { TextInput } from "../../shared/ui/form/text-input";
import { useI18n } from "../i18n/use-i18n";
import { isImageFile } from "./image-file-preview";
import { getUnsavedEditorPaths } from "./unsaved-editor-changes";
import { isAbsoluteFilePath } from "./workspace-path-utils";

/**
 * 通过服务器绝对路径打开现有文件，不切换工作区。
 * @param props 弹层状态、初始路径及文件选择和关闭回调
 * @returns 文件打开对话框
 */
export function OpenFileDialog({ open, initialPath = "", onSelectFile, onClose }: {
  open: boolean;
  initialPath?: string;
  onSelectFile: (path: string) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [path, setPath] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const formId = useId();
  const inputId = useId();
  const errorId = useId();
  useEffect(() => {
    if (!open) return;
    setPath(isAbsoluteFilePath(initialPath) ? initialPath : "");
    setError(null);
  }, [open, initialPath]);

  /** 验证文件可读后交给现有编辑器。@returns 文件打开完成信号 */
  const openFile = async () => {
    const requested = path.trim();
    if (pending || !requested) return;
    if (!isAbsoluteFilePath(requested)) {
      setError(new Error(t("Enter an absolute file path.", "请输入文件绝对路径。")));
      return;
    }
    // 1. 当前编辑器切换文件时会卸载，先保护尚未保存的内容
    if (getUnsavedEditorPaths().some((dirtyPath) => dirtyPath !== requested)) {
      setError(new Error(t("Save your current edits before opening another file.", "请先保存当前修改，再打开其他文件。")));
      return;
    }
    setPending(true);
    setError(null);
    try {
      // 2. 复用文件接口的类型、大小与访问校验，文本快照交给编辑器缓存
      if (isImageFile(requested)) {
        const response = await fetch(api.workspace.imageUrl(requested), { credentials: "same-origin" });
        if (!response.ok) {
          const body = await response.json().catch(() => null) as { error?: string } | null;
          throw new ApiError(body?.error ?? `HTTP ${response.status}`);
        }
        await response.body?.cancel();
      } else {
        const file = await api.workspace.file(requested);
        queryClient.setQueryData(["file", requested], file);
      }
      onSelectFile(requested);
      onClose();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to open file", "打开文件失败"));
    } finally {
      setPending(false);
    }
  };

  return <Modal open={open} title={t("Open file", "打开文件")} description={t("Enter an absolute path to an existing file on the server, including files outside this workspace.", "输入服务器上现有文件的绝对路径，也可打开当前工作区之外的文件。")} size="small" initialFocusRef={inputRef} onClose={() => { if (!pending) onClose(); }} footer={<>
    <Button disabled={pending} onClick={onClose}>{t("Cancel", "取消")}</Button>
    <Button variant="primary" type="submit" form={formId} disabled={pending || !path.trim()}>{pending ? t("Opening", "正在打开") : t("Open", "打开")}</Button>
  </>}>
    <form id={formId} className="grid gap-3" onSubmit={(event) => { event.preventDefault(); void openFile(); }}>
      <label htmlFor={inputId} className="text-sm">{t("Absolute file path", "文件绝对路径")}</label>
      <TextInput ref={inputRef} id={inputId} value={path} onChange={(event) => setPath(event.target.value)} onFocus={(event) => event.currentTarget.select()} disabled={pending} placeholder="/path/to/file" spellCheck={false} autoComplete="off" aria-describedby={error ? errorId : undefined} />
      {error && <p id={errorId} className="m-0 text-sm text-[var(--danger)]" role="alert">{error.message}</p>}
    </form>
  </Modal>;
}
