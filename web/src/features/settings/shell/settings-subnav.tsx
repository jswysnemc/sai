import { NavLink } from "react-router-dom";
import type { SettingsSectionId, SettingsSubviewMeta } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";

type SettingsSubnavProps = {
  sectionId: SettingsSectionId;
  subviews: SettingsSubviewMeta[];
};

/**
 * 渲染分区的二级子页导航。
 *
 * 子页是真实路由（/settings/:sectionId/:subview），刷新与分享后
 * 停留在同一子页；激活态由路由匹配派生，不再依赖分区本地 state。
 *
 * @param props 分区 id 与子页注册列表
 * @returns 子页标签导航
 */
export function SettingsSubnav({ sectionId, subviews }: SettingsSubnavProps) {
  const { t } = useI18n();
  return (
    <nav className="settings-tabs settings-subnav" aria-label={t("Section pages", "分区子页")}>
      {subviews.map((item) => (
        <NavLink
          key={item.id}
          to={`/settings/${sectionId}/${item.id}`}
          className={({ isActive }) => (isActive ? "active" : undefined)}
        >
          {t(item.labelEn, item.labelZh)}
        </NavLink>
      ))}
    </nav>
  );
}
