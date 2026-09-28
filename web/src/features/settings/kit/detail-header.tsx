import type { ReactNode } from "react";
import { MoreHorizontal } from "../../../shared/ui/icons";
import { ActionMenu, type ActionMenuItem } from "../../../shared/ui/menu/action-menu";
import { useI18n } from "../../i18n/use-i18n";
import { cx } from "./class-names";
import "./detail-header.css";

type DetailHeaderProps = {
  title: ReactNode;
  /** 标题下方的次要信息，例如标识或文件路径 */
  subtitle?: ReactNode;
  /** 次要信息使用等宽字体 */
  subtitleMono?: boolean;
  icon?: ReactNode;
  /** 紧跟标题的状态徽标 */
  badges?: ReactNode;
  /** 常用操作 */
  actions?: ReactNode;
  /** 低频与危险操作，收进「更多」菜单 */
  menuItems?: ActionMenuItem[];
};

/**
 * 渲染对象详情标题：名称与状态在左，常用操作与更多菜单在右。
 *
 * 删除等危险操作放进更多菜单，与常用操作隔开，降低误触概率。
 *
 * @param props 标题、次要信息、图标、徽标、操作与菜单项
 * @returns 详情标题行
 */
export function DetailHeader({ title, subtitle, subtitleMono, icon, badges, actions, menuItems }: DetailHeaderProps) {
  const { t } = useI18n();
  return (
    <header className="sk-detail-header">
      <div className="sk-detail-heading">
        {icon && <span className="sk-detail-icon">{icon}</span>}
        <div className="sk-detail-copy">
          <div className="sk-detail-title-row">
            <h2>{title}</h2>
            {badges}
          </div>
          {subtitle && <span className={cx("sk-detail-subtitle", subtitleMono && "is-mono")}>{subtitle}</span>}
        </div>
      </div>
      {(actions || (menuItems && menuItems.length > 0)) && (
        <div className="sk-detail-actions">
          {actions}
          {menuItems && menuItems.length > 0 && (
            <ActionMenu label={t("More actions", "更多操作")} trigger={<MoreHorizontal size={16} />} items={menuItems} />
          )}
        </div>
      )}
    </header>
  );
}
