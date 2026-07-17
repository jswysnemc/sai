import { type CSSProperties, type RefObject, useLayoutEffect, useState } from "react";
import { calculateAnchoredPopoverPosition } from "./anchored-popover-position";

type UseAnchoredPopoverOptions = {
  open: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  preferredWidth?: number;
  minimumWidth?: number;
  align?: "left" | "right";
  maxHeight?: number;
};

/**
 * 跟随触发器计算 Portal 弹层的固定定位样式。
 *
 * @param options 打开状态、触发器引用和菜单尺寸
 * @returns 可直接应用到弹层的固定定位样式
 */
export function useAnchoredPopover(options: UseAnchoredPopoverOptions): CSSProperties {
  const [style, setStyle] = useState<CSSProperties>({ position: "fixed", top: 0, left: 0, width: 0 });

  useLayoutEffect(() => {
    if (!options.open) return;

    /** 根据触发器和当前视口更新弹层位置。 */
    const updatePosition = () => {
      const rect = options.anchorRef.current?.getBoundingClientRect();
      if (!rect) return;
      const position = calculateAnchoredPopoverPosition(rect, {
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
        preferredWidth: options.preferredWidth ?? rect.width,
        minimumWidth: options.minimumWidth ?? rect.width,
        align: options.align ?? "left",
        maxHeight: options.maxHeight
      });
      setStyle({ position: "fixed", ...position });
    };

    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [options.align, options.anchorRef, options.maxHeight, options.minimumWidth, options.open, options.preferredWidth]);

  return style;
}
