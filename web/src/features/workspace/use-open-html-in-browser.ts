import { useEffect, type Dispatch, type SetStateAction } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import { useToast, type ToastNotice } from "../../shared/ui/notify/notify";
import { requestBuiltInBrowserNavigation } from "../browser/browser-preview-navigation";
import { useI18n } from "../i18n/use-i18n";
import { OPEN_HTML_IN_BROWSER_EVENT, isHtmlFile } from "./html-file";
import { createWorkspacePanelTab, type PaneTab, type WorkspacePanelTab } from "./workspace-tab";

/**
 * 监听 HTML 右键打开请求：创建预览地址，切到内置浏览器并导航。
 *
 * @param setTabs 更新工作区页签
 * @param setActiveTabId 激活页签
 * @param onActiveTypeChange 同步当前面板类型
 * @returns 失败提示
 */
export function useOpenHtmlInBrowser(
  setTabs: Dispatch<SetStateAction<WorkspacePanelTab[]>>,
  setActiveTabId: Dispatch<SetStateAction<string | null>>,
  onActiveTypeChange: (tab: PaneTab | null) => void
): { notice: ToastNotice | null; dismissToast: () => void } {
  const { locale } = useI18n();
  const { notice, showToast, dismissToast } = useToast();

  useEffect(() => {
    /**
     * 打开右键选中的 HTML 文件。
     *
     * @param event 携带文件路径的窗口事件
     * @returns 无返回值
     */
    const open = (event: Event) => {
      const path = (event as CustomEvent<string>).detail;
      if (typeof path !== "string" || !isHtmlFile(path)) return;
      void openHtml(path);
    };

    /**
     * 签发预览地址并激活内置浏览器页签。
     *
     * @param path 文件路径
     * @returns 打开完成后的 Promise
     */
    const openHtml = async (path: string) => {
      try {
        const preview = await api.workspace.htmlPreview(path);
        const url = new URL(preview.path, window.location.origin).toString();
        // 1. 先记下地址，浏览器面板连上后会立刻导航
        requestBuiltInBrowserNavigation(url);
        // 2. 复用已有浏览器页签，没有时新建
        setTabs((current) => {
          const existing = current.find((tab) => tab.type === "browser");
          if (existing) {
            setActiveTabId(existing.id);
            return current;
          }
          const created = createWorkspacePanelTab("browser", {}, locale);
          setActiveTabId(created.id);
          return [...current, created];
        });
        onActiveTypeChange("browser");
      } catch (reason) {
        showToast(
          toDisplayError(reason, "Could not open the HTML file", "无法在内置浏览器中打开这个 HTML 文件").message,
          "error"
        );
      }
    };

    window.addEventListener(OPEN_HTML_IN_BROWSER_EVENT, open);
    return () => window.removeEventListener(OPEN_HTML_IN_BROWSER_EVENT, open);
  }, [locale, onActiveTypeChange, setActiveTabId, setTabs, showToast]);

  return { notice, dismissToast };
}
