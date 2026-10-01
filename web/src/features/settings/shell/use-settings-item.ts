import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";

/**
 * 【Web 设置】【对象定位】把选中对象写入地址，失效标识回退到首项。
 * @param ids 当前领域中合法的对象标识
 * @param fallbackId 未指定对象时优先选择的标识
 * @returns 当前标识与选择回调
 */
export function useSettingsItem(ids: readonly string[], fallbackId?: string): [string, (id: string) => void] {
  const [params, setParams] = useSearchParams();
  const [pendingId, setPendingId] = useState<string | null>(null);
  const requested = params.get("item") ?? "";
  const target = pendingId ?? requested;
  const selected = ids.includes(target) ? target : fallbackId && ids.includes(fallbackId) ? fallbackId : ids[0] ?? "";

  const select = useCallback((id: string) => setPendingId(id), []);

  useEffect(() => {
    // 1. 【Web 设置】【对象定位】本地选择与配置同批更新，路由过渡期间不按旧 ID 回退
    if (pendingId !== null && requested === pendingId) {
      setPendingId(null);
      return;
    }
    // 2. 异步列表尚未就绪时保留地址中的对象，避免刷新丢失选择
    if (pendingId === null && (ids.length === 0 || selected === requested)) return;
    const id = pendingId ?? selected;
    setParams((current) => {
      const next = new URLSearchParams(current);
      if (id) next.set("item", id);
      else next.delete("item");
      return next;
    }, { replace: true });
  }, [ids.length, pendingId, requested, selected, setParams]);

  return [selected, select];
}
