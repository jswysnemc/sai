import { useRef, useState } from "react";
import { Download, Paperclip, X } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import type { BrowserDownload, BrowserFileChooser } from "./browser-protocol";

/**
 * 把一个文件上传到服务端临时目录。
 *
 * @param file 用户选择的文件
 * @returns 上传 ID
 */
async function uploadFile(file: File): Promise<string> {
  const response = await fetch(`/api/browser/uploads?name=${encodeURIComponent(file.name)}`, {
    method: "POST",
    credentials: "same-origin",
    body: file
  });
  const body = (await response.json().catch(() => null)) as { id?: string; error?: string } | null;
  if (!response.ok || !body?.id) throw new Error(body?.error ?? `HTTP ${response.status}`);
  return body.id;
}

type BrowserFilePromptProps = {
  chooser: BrowserFileChooser;
  onAnswer: (uploads: string[]) => void;
};

/**
 * 页面请求选择文件时的提示条：用户在本机选择文件，上传后交给页面的文件输入框。
 *
 * @param props 文件选择请求与回复回调
 * @returns 提示条
 */
export function BrowserFilePrompt({ chooser, onAnswer }: BrowserFilePromptProps) {
  const { t } = useI18n();
  const inputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** 上传选中的文件并回复页面。 */
  const submit = async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      const ids = await Promise.all([...files].map(uploadFile));
      onAnswer(ids);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="browser-banner" role="status">
      <Paperclip size={14} aria-hidden />
      <span>{chooser.multiple ? t("The page asks for files", "网页请求选择文件") : t("The page asks for a file", "网页请求选择一个文件")}</span>
      {error && <span className="browser-banner-error">{error}</span>}
      <input
        ref={inputRef}
        type="file"
        hidden
        multiple={chooser.multiple}
        accept={chooser.accept || undefined}
        onChange={(event) => void submit(event.target.files)}
      />
      <Button size="small" variant="primary" disabled={busy} onClick={() => inputRef.current?.click()}>
        {busy ? t("Uploading…", "正在上传…") : t("Choose files", "选择文件")}
      </Button>
      <Button size="small" variant="ghost" disabled={busy} onClick={() => onAnswer([])}>
        {t("Cancel", "取消")}
      </Button>
    </div>
  );
}

type BrowserDownloadsProps = {
  downloads: BrowserDownload[];
  onDismiss: (guid: string) => void;
};

/**
 * 页面下载列表：下载中显示进度，完成后可保存到本机。
 *
 * @param props 下载记录与移除回调
 * @returns 下载提示条；没有下载时为空
 */
export function BrowserDownloads({ downloads, onDismiss }: BrowserDownloadsProps) {
  const { t } = useI18n();
  if (downloads.length === 0) return null;
  return (
    <div className="browser-downloads" aria-live="polite">
      {downloads.map((item) => (
        <div key={item.guid} className="browser-banner">
          <Download size={14} aria-hidden />
          <span className="browser-banner-name" title={item.url}>{item.file_name}</span>
          {item.state === "completed" ? (
            <a className="ui-button primary ui-button-small" href={`/api/browser/downloads/${encodeURIComponent(item.guid)}`} download={item.file_name}>
              {t("Save", "保存")}
            </a>
          ) : (
            <span className="browser-banner-meta">
              {item.state === "canceled" ? t("Canceled", "已取消") : t("Downloading…", "正在下载…")}
            </span>
          )}
          <button type="button" className="browser-banner-close" aria-label={t("Dismiss", "关闭提示")} onClick={() => onDismiss(item.guid)}>
            <X size={12} aria-hidden />
          </button>
        </div>
      ))}
    </div>
  );
}
