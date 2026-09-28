import { useCallback, useEffect } from "react";
import { useSearchParams } from "react-router-dom";

/**
 * 【Web 设置】【对象定位】把选中对象写入地址，失效标识回退到首项。
 * @param ids 当前领域中合法的对象标识
 * @param fallbackId 未指定对象时优先选择的标识
 * @returns 当前标识与选择回调
 */
export function useSettingsItem(ids: readonly string[], fallbackId?: string): [string, (id: string) => void] {
  const [params, setParams] = useSearchParams();
  const requested = params.get("item") ?? "";
  const selected = ids.includes(requested) ? requested : fallbackId && ids.includes(fallbackId) ? fallbackId : ids[0] ?? "";

  const select = useCallback((id: string) => {
    setParams((current) => {
      const next = new URLSearchParams(current);
      if (id) next.set("item", id);
      else next.delete("item");
      return next;
    }, { replace: true });
  }, [setParams]);

  useEffect(() => {
    if (selected !== requested) select(selected);
  }, [requested, select, selected]);

  return [selected, select];
}
