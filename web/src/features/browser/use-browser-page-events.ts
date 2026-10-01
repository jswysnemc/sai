import { useCallback, useEffect, useState } from "react";
import { useI18n } from "../i18n/use-i18n";
import { sendPickedElementToChat } from "./browser-element-context";
import type {
  BrowserClientMessage,
  BrowserDialog,
  BrowserDownload,
  BrowserFileChooser,
  BrowserSelectPopup,
  BrowserServerMessage
} from "./browser-protocol";
import type { BrowserPageMessageHandler } from "./use-browser-session";

/** 面板保留的最近下载数量。 */
const MAX_DOWNLOADS = 5;
/** 复制结果提示的显示时长。 */
const NOTICE_MS = 2_500;

/**
 * 管理页面交互状态：对话框、下拉菜单、文件选择、下载、复制与元素选择。
 *
 * @param setPageHandler 注册页面消息回调
 * @param send 控制消息发送方法
 * @returns 各类交互的当前状态与操作方法
 */
export function useBrowserPageEvents(
  setPageHandler: (handler: BrowserPageMessageHandler | null) => void,
  send: (message: BrowserClientMessage) => void
) {
  const { t } = useI18n();
  const [dialog, setDialog] = useState<BrowserDialog | null>(null);
  const [selectPopup, setSelectPopup] = useState<BrowserSelectPopup | null>(null);
  const [fileChooser, setFileChooser] = useState<BrowserFileChooser | null>(null);
  const [downloads, setDownloads] = useState<BrowserDownload[]>([]);
  const [picking, setPicking] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    /** 分派一条页面交互消息。 */
    const handle = (message: BrowserServerMessage) => {
      switch (message.type) {
        case "dialog":
          setDialog(message.dialog);
          break;
        case "select_popup":
          setSelectPopup(message.popup);
          break;
        case "file_chooser":
          setFileChooser(message.chooser);
          break;
        case "download":
          setDownloads((current) => [message.download, ...current.filter((item) => item.guid !== message.download.guid)].slice(0, MAX_DOWNLOADS));
          break;
        case "clipboard":
          // 页面内复制写入本机剪贴板；非安全上下文没有剪贴板接口时提示失败
          void navigator.clipboard?.writeText(message.text).then(
            () => setNotice(t("Copied to clipboard", "已复制到剪贴板")),
            () => setNotice(t("Clipboard is not available on this connection", "当前连接无法写入剪贴板"))
          );
          break;
        case "picked":
          setPicking(false);
          if (message.element) sendPickedElementToChat(message.element);
          break;
        default:
          break;
      }
    };
    setPageHandler(handle);
    return () => setPageHandler(null);
  }, [setPageHandler, t]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [notice]);

  /** 切换元素选择：进行中则取消，否则开始。 */
  const togglePicking = useCallback(() => {
    if (picking) {
      send({ type: "pick_cancel" });
      setPicking(false);
      return;
    }
    send({ type: "pick_start", labels: [t("Background", "背景"), t("Color", "颜色"), t("Font", "字体")] });
    setPicking(true);
  }, [picking, send, t]);

  /** 回复对话框。 */
  const answerDialog = useCallback((accept: boolean, promptText?: string) => {
    send({ type: "dialog_reply", accept, ...(promptText !== undefined ? { prompt_text: promptText } : {}) });
    setDialog(null);
  }, [send]);

  /** 回复下拉菜单；index 为空表示关闭菜单不改动。 */
  const answerSelect = useCallback((index: number | null) => {
    if (index !== null) send({ type: "select_reply", index });
    setSelectPopup(null);
  }, [send]);

  /** 回复文件选择；空列表表示取消。 */
  const answerFiles = useCallback((uploads: string[]) => {
    send({ type: "file_chooser_reply", uploads });
    setFileChooser(null);
  }, [send]);

  /** 从列表中移除一条下载。 */
  const dismissDownload = useCallback((guid: string) => {
    setDownloads((current) => current.filter((item) => item.guid !== guid));
  }, []);

  return {
    dialog,
    selectPopup,
    fileChooser,
    downloads,
    picking,
    notice,
    togglePicking,
    answerDialog,
    answerSelect,
    answerFiles,
    dismissDownload
  };
}
