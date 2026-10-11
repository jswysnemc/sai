import type { ReactNode } from "react";
import { cx } from "./class-names";
import "./settings-row.css";

/** 字段行排布：始终横排、始终上下、或窄屏上下宽屏横排。 */
export type SettingsRowLayout = "inline" | "stack" | "responsive";

type SettingsRowProps = {
  title: ReactNode;
  description?: ReactNode;
  /** 标题旁的辅助操作 */
  aside?: ReactNode;
  /** 控件，宽屏时靠右 */
  children?: ReactNode;
  /** 标题与控件之下的通栏内容，例如错误详情 */
  footer?: ReactNode;
  layout?: SettingsRowLayout;
  /** 把标题渲染为指向控件的 label */
  htmlFor?: string;
  titleId?: string;
  descriptionId?: string;
  className?: string;
  id?: string;
  /** 对应的配置键，只出现在标签提示与搜索索引中 */
  configKey?: string;
  /** 控件最大宽度档位 */
  size?: string;
  "data-testid"?: string;
};

/**
 * 渲染设置字段行：左侧 13px 标题与 12px 说明，右侧控件。
 *
 * @param props 标题、说明、控件、排布与无障碍属性
 * @returns 字段行
 */
export function SettingsRow({
  title,
  description,
  aside,
  children,
  footer,
  layout = "responsive",
  htmlFor,
  titleId,
  descriptionId,
  className,
  id,
  configKey,
  size,
  "data-testid": testId
}: SettingsRowProps) {
  const titleClass = "sk-row-title";
  const titleNode = htmlFor ? (
    <label id={titleId} htmlFor={htmlFor} className={titleClass} title={configKey}>{title}</label>
  ) : (
    <div id={titleId} className={titleClass} title={configKey}>{title}</div>
  );
  return (
    <div
      id={id}
      data-testid={testId}
      data-size={size}
      data-config-key={configKey}
      className={cx("sk-row", `is-${layout}`, className)}
    >
      <div className="sk-row-main">
        <div className="sk-row-copy">
          {titleNode}
          {description ? (
            <div id={descriptionId} className="sk-row-desc">{description}</div>
          ) : null}
          {aside ? <div className="sk-row-aside">{aside}</div> : null}
        </div>
        {children ? <div className="sk-row-control">{children}</div> : null}
      </div>
      {footer ? <div className="sk-row-footer">{footer}</div> : null}
    </div>
  );
}
