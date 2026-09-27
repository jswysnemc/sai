import type { ReactNode } from "react";
import "./collapse.css";

type CollapseProps = {
  open: boolean;
  className?: string;
  children: ReactNode;
};

/**
 * 以网格行高过渡实现平滑展开与收缩，收起后内容不可聚焦。
 *
 * @param props 是否展开、附加类名与内容
 * @returns 可折叠容器
 */
export function Collapse({ open, className, children }: CollapseProps) {
  const classes = `ui-collapse${open ? " is-open" : ""}${className ? ` ${className}` : ""}`;
  return (
    <div className={classes} inert={!open}>
      <div className="ui-collapse-inner">{children}</div>
    </div>
  );
}
