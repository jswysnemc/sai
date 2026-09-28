import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { fieldAnchorId } from "../search/field-anchor";

/** 等待字段渲染的最长轮询次数，每次间隔 100ms。 */
const FOCUS_ATTEMPTS = 40;

/**
 * 【Web 设置】【字段定位】处理地址中的 focus 参数。
 *
 * 分区可能还在加载配置，因此按固定间隔轮询字段锚点；找到后滚动到视口中部
 * 并短暂高亮，随后从地址中移除 focus 参数，保留其余参数。
 *
 * @param routeKey 当前分区与子页组合，变化时重新定位
 * @returns 未显示字段的标识，用于展示配置条件提示
 */
export function useFieldFocus(routeKey: string): string | null {
  const [params, setParams] = useSearchParams();
  const focus = params.get("focus");
  const [missing, setMissing] = useState<string | null>(null);
  useEffect(() => setMissing(null), [routeKey]);

  useEffect(() => {
    if (!focus) return;
    setMissing(null);
    let attempts = 0;
    let timer = 0;

    /**
     * 从地址中移除 focus 参数。
     *
     * @returns 无返回值
     */
    const clearFocusParam = () => {
      setParams((current) => {
        const next = new URLSearchParams(current);
        next.delete("focus");
        return next;
      }, { replace: true });
    };

    /**
     * 查找字段并定位；未渲染时继续等待。
     *
     * @returns 无返回值
     */
    const locate = () => {
      const element = document.getElementById(fieldAnchorId(focus));
      if (element) {
        // 1. 展开包含目标字段的折叠组，再执行滚动
        let ancestor = element.parentElement;
        while (ancestor) {
          if (ancestor instanceof HTMLDetailsElement) ancestor.open = true;
          ancestor = ancestor.parentElement;
        }
        // 1. 滚动到视口中部并重新触发高亮动画
        element.scrollIntoView({ block: "center", behavior: "smooth" });
        element.classList.remove("sk-flash");
        void element.offsetWidth;
        element.classList.add("sk-flash");
        window.setTimeout(() => element.classList.remove("sk-flash"), 1700);
        clearFocusParam();
        return;
      }
      // 2. 超过等待上限仍未找到时放弃，避免参数残留
      attempts += 1;
      if (attempts >= FOCUS_ATTEMPTS) {
        setMissing(focus);
        clearFocusParam();
        return;
      }
      timer = window.setTimeout(locate, 100);
    };

    locate();
    return () => window.clearTimeout(timer);
  }, [focus, routeKey, setParams]);
  return missing;
}
