import { useEffect, useState } from 'react';

const INITIAL = { maximized: false, canGoBack: false, canGoForward: false, loading: true,
  title: 'Sai', appearance: {} };

/**
 * 【桌面界面】【状态订阅】同步窗口、导航与 Sai 主题，并在卸载时清理订阅
 * @returns {object} 当前桌面状态
 */
export function useDesktopState() {
  const [state, setState] = useState(INITIAL);
  useEffect(() => {
    const update = (value) => {
      setState(value);
      for (const [key, color] of Object.entries(value.appearance || {})) {
        document.documentElement.style.setProperty(`--${key}`, color);
      }
    };
    const unsubscribe = window.saiDesktop.onState(update);
    window.saiDesktop.getState().then(update);
    return unsubscribe;
  }, []);
  return state;
}
