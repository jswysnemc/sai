import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "./class-names";
import "./settings-group.css";

type SettingsGroupProps = {
  className?: string;
  children?: ReactNode;
} & Pick<HTMLAttributes<HTMLDivElement>, "id" | "role" | "aria-label" | "aria-labelledby">;

/**
 * 渲染设置分组卡片：圆角细边框，子项之间以细分隔线区分。
 *
 * 不使用 overflow:hidden，避免行内下拉菜单被裁切。
 *
 * @param props 无障碍属性、类名与子节点
 * @returns 分组卡片
 */
export function SettingsGroup({ className, children, ...rest }: SettingsGroupProps) {
  return (
    <div {...rest} className={cx("sk-group", className)}>
      {children}
    </div>
  );
}
