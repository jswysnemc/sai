import { useLayoutEffect, useState, type RefObject } from "react";

/**
 * 把固定定位菜单钳在视口内，避免贴边时被裁切。
 *
 * @param x 指针横坐标
 * @param y 指针纵坐标
 * @param ref 菜单节点
 * @returns 钳制后的 left 与 top
 */
export function useClampedMenuPosition(x: number, y: number, ref: RefObject<HTMLElement | null>): { left: number; top: number } {
  const [position, setPosition] = useState({ left: x, top: y });
  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    const { width, height } = node.getBoundingClientRect();
    const margin = 8;
    setPosition({
      left: Math.max(margin, Math.min(x, window.innerWidth - width - margin)),
      top: Math.max(margin, Math.min(y, window.innerHeight - height - margin))
    });
  }, [x, y, ref]);
  return position;
}
