import { useCallback, useEffect, useRef, useState } from "react";
import {
  canGoBack,
  canGoForward,
  EMPTY_FILE_NAVIGATION_HISTORY,
  goBack,
  goForward,
  recordFileVisit
} from "./file-navigation-history";

/**
 * 跟踪编辑器文件访问历史，提供后退/前进导航。
 *
 * 由后退/前进触发的选中变化不再入栈，避免历史自我循环。
 *
 * @param selectedFile 当前选中的文件路径
 * @param onSelectFile 打开文件回调
 * @returns 可用状态与导航方法
 */
export function useFileNavigationHistory(
  selectedFile: string | null,
  onSelectFile: (path: string) => void
) {
  const [history, setHistory] = useState(EMPTY_FILE_NAVIGATION_HISTORY);
  const navigationTargetRef = useRef<string | null>(null);

  useEffect(() => {
    if (!selectedFile) return;
    if (navigationTargetRef.current === selectedFile) {
      navigationTargetRef.current = null;
      return;
    }
    setHistory((current) => recordFileVisit(current, selectedFile));
  }, [selectedFile]);

  const back = useCallback(() => {
    const result = goBack(history);
    if (!result) return;
    setHistory(result.history);
    if (selectedFile !== result.path) {
      navigationTargetRef.current = result.path;
      onSelectFile(result.path);
    }
  }, [history, onSelectFile, selectedFile]);

  const forward = useCallback(() => {
    const result = goForward(history);
    if (!result) return;
    setHistory(result.history);
    if (selectedFile !== result.path) {
      navigationTargetRef.current = result.path;
      onSelectFile(result.path);
    }
  }, [history, onSelectFile, selectedFile]);

  return {
    canBack: canGoBack(history),
    canForward: canGoForward(history),
    back,
    forward
  };
}
