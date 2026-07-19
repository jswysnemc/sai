import { useEffect, useRef } from "react";

const AUTOFETCH_INTERVAL_MS = 180_000;

type GitAutofetchOptions = {
  enabled: boolean;
  ready: boolean;
  remoteConfigured: boolean;
  busy: boolean;
  hasOperation: boolean;
  onFetch: () => Promise<unknown>;
};

/**
 * 判断当前仓库状态是否允许执行自动获取。
 *
 * @param options 配置、仓库和页面运行状态
 * @returns 是否允许执行 fetch
 */
export function canRunGitAutofetch(
  options: Omit<GitAutofetchOptions, "onFetch"> & { pageVisible: boolean; online: boolean }
): boolean {
  return options.enabled
    && options.ready
    && options.remoteConfigured
    && !options.busy
    && !options.hasOperation
    && options.pageVisible
    && options.online;
}

/**
 * 在仓库空闲且页面可见时定期获取远端引用。
 *
 * @param options 配置、仓库状态和 fetch 回调
 * @returns 无返回值
 */
export function useGitAutofetch(options: GitAutofetchOptions) {
  const fetchRef = useRef(options.onFetch);
  const runningRef = useRef(false);
  fetchRef.current = options.onFetch;

  useEffect(() => {
    if (!options.enabled || !options.ready || !options.remoteConfigured) return undefined;

    /** 到达周期后按实时页面状态执行一次 fetch。 */
    const tick = async () => {
      const allowed = canRunGitAutofetch({
        enabled: options.enabled,
        ready: options.ready,
        remoteConfigured: options.remoteConfigured,
        busy: options.busy,
        hasOperation: options.hasOperation,
        pageVisible: typeof document === "undefined" || document.visibilityState === "visible",
        online: typeof navigator === "undefined" || navigator.onLine
      });
      if (!allowed || runningRef.current) return;
      runningRef.current = true;
      try {
        await fetchRef.current();
      } finally {
        runningRef.current = false;
      }
    };

    const interval = window.setInterval(() => void tick(), AUTOFETCH_INTERVAL_MS);
    return () => window.clearInterval(interval);
  }, [options.busy, options.enabled, options.hasOperation, options.ready, options.remoteConfigured]);
}
