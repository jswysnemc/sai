import { useEffect, useState } from "react";

/**
 * 跟踪系统的"减少动效"偏好。
 *
 * 工具卡片的文本切换与流光渐变都是装饰性动画，开启该偏好的用户
 * 需要的是静态呈现而非更慢的动画。
 *
 * @returns 系统是否要求减少动效
 */
export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const sync = () => setReduced(query.matches);
    sync();
    query.addEventListener("change", sync);
    return () => query.removeEventListener("change", sync);
  }, []);

  return reduced;
}
