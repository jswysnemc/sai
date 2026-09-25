import { ChevronDown, ChevronUp } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "../../../../api/client";
import { useI18n } from "../../../i18n/use-i18n";
import { contextLinesFromFile } from "./diff-fold-lines";
import type { DiffLine } from "./diff-model";

const fileText = new Map<string, Promise<string>>();

type DiffHunkFoldProps = {
  path: string;
  line: DiffLine;
  className: string;
  /** hunk 头里的函数名等附加说明 */
  detail?: string;
  children: (lines: DiffLine[]) => ReactNode;
};

/**
 * 渲染可点开的 hunk 间隔折叠条。
 *
 * 补丁里没有这些未修改行，第一次点开时读取工作区文件并按行号回填。
 *
 * @param props 文件路径、省略标记、样式和展开后的行渲染
 * @returns 折叠条；展开后在其后接上省略行
 */
export function DiffHunkFold({ path, line, className, detail, children }: DiffHunkFoldProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [lines, setLines] = useState<DiffLine[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const count = line.foldedCount ?? 0;

  const toggle = async () => {
    if (open) {
      setOpen(false);
      return;
    }
    if (!lines) {
      if (!path || !line.foldStart || !line.foldEnd) {
        setFailed(true);
        setOpen(true);
        return;
      }
      setBusy(true);
      try {
        const content = await loadWorkspaceFile(path);
        setLines(contextLinesFromFile(content, line));
        setFailed(false);
      } catch {
        setFailed(true);
      } finally {
        setBusy(false);
      }
    }
    setOpen(true);
  };

  const label = failed
    ? t("Could not read these lines", "读不到这些未修改行")
    : busy
      ? t("Reading omitted lines", "正在读取未修改行")
      : open
        ? t("Fold unchanged lines", "折叠未修改内容")
        : t(`${count} unchanged lines`, `${count} 行未修改内容`);

  return (
    <>
      <button type="button" className={className} aria-expanded={open} onClick={() => void toggle()}>
        {open ? <ChevronUp size={13} aria-hidden /> : <ChevronDown size={13} aria-hidden />}
        <span>{label}</span>
        {detail && <code>{detail}</code>}
      </button>
      {open && lines && children(lines)}
    </>
  );
}

/**
 * 读取并缓存工作区文件全文，同一路径的多次展开共用一次请求。
 *
 * @param path 工作区相对路径
 * @returns 文件全文
 */
function loadWorkspaceFile(path: string): Promise<string> {
  const cached = fileText.get(path);
  if (cached) return cached;
  const pending = api.workspace.file(path).then((file) => file.content).catch((error: unknown) => {
    fileText.delete(path);
    throw error;
  });
  fileText.set(path, pending);
  return pending;
}
