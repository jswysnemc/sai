import type { ReactNode } from "react";
import { cx } from "./class-names";
import "./settings-page-header.css";

type SettingsPageHeaderProps = {
  title: ReactNode;
  description?: ReactNode;
  /** 右侧操作，例如即时生效说明 */
  action?: ReactNode;
  /** 标题元素标识，供区域 aria-labelledby 引用 */
  titleId?: string;
  className?: string;
};

/**
 * 渲染设置分区页头：22px 标题与 13px 说明，可选右侧操作。
 *
 * 标题与侧栏导航使用同一组分区文案，避免两处命名不一致。
 *
 * @param props 标题、说明、操作与类名
 * @returns 分区页头
 */
export function SettingsPageHeader({ title, description, action, titleId, className }: SettingsPageHeaderProps) {
  return (
    <header className={cx("settings-page-header", className)}>
      <div className="settings-page-header-copy">
        <h1 id={titleId} className="settings-page-header-title">{title}</h1>
        {description ? <p className="settings-page-header-desc">{description}</p> : null}
      </div>
      {action ? <div className="settings-page-header-action">{action}</div> : null}
    </header>
  );
}
