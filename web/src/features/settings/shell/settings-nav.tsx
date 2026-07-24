import { useMemo, useState } from "react";
import { NavLink } from "react-router-dom";
import {
  filterSettingsSections,
  groupSettingsSections
} from "../settings-registry";
import type { SettingsSectionId } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";

type SettingsNavProps = {
  activeSection: SettingsSectionId;
};

/**
 * 渲染分组设置导航与搜索过滤。
 *
 * @param props 当前激活 section
 * @returns 侧栏 / 移动端横向导航
 */
export function SettingsNav({ activeSection }: SettingsNavProps) {
  const { t, locale } = useI18n();
  const [query, setQuery] = useState("");

  // 1. 按关键字过滤，再按分组归类
  const grouped = useMemo(() => {
    const filtered = filterSettingsSections(query, locale);
    return groupSettingsSections(filtered);
  }, [locale, query]);

  return (
    <nav className="settings-navigation" aria-label={t("Settings categories", "设置分类")}>
      <div className="settings-navigation-label">{t("Settings categories", "设置分类")}</div>
      <label className="settings-nav-search">
        <span className="sr-only">{t("Search settings", "搜索设置")}</span>
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("Search settings", "搜索设置")}
          aria-label={t("Search settings", "搜索设置")}
        />
      </label>
      {grouped.length === 0 && (
        <div className="settings-nav-empty">{t("No matching settings", "没有匹配的设置项")}</div>
      )}
      {grouped.map(({ group, sections }) => (
        <div className="settings-nav-group" key={group.id}>
          <div className="settings-nav-group-label">{t(group.labelEn, group.labelZh)}</div>
          {sections.map(({ id, labelEn, labelZh, descriptionEn, descriptionZh, icon: Icon }) => (
            <NavLink
              key={id}
              to={`/settings/${id}`}
              className={({ isActive }) => (isActive || id === activeSection ? "active" : undefined)}
            >
              <Icon size={15} />
              <span>
                <strong>{t(labelEn, labelZh)}</strong>
                <small>{t(descriptionEn, descriptionZh)}</small>
              </span>
            </NavLink>
          ))}
        </div>
      ))}
    </nav>
  );
}
