import { useEffect, useMemo, useRef, useState } from "react";
import { NavLink } from "react-router-dom";
import { Search } from "lucide-react";
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
  const navigationRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const media = window.matchMedia("(width < 48rem)");
    let frame = 0;
    /**
     * 在横向分类栏中显示当前分类，同时移除已隐藏搜索框中的筛选条件。
     * @returns 无返回值
     */
    const revealSelection = () => {
      if (!media.matches) return;
      setQuery("");
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => navigationRef.current?.querySelector("a.active")?.scrollIntoView({ block: "nearest", inline: "center" }));
    };
    revealSelection();
    media.addEventListener("change", revealSelection);
    return () => {
      cancelAnimationFrame(frame);
      media.removeEventListener("change", revealSelection);
    };
  }, [activeSection, locale]);

  // 1. 按关键字过滤，再按分组归类
  const grouped = useMemo(() => {
    const filtered = filterSettingsSections(query, locale);
    return groupSettingsSections(filtered);
  }, [locale, query]);

  return (
    <nav ref={navigationRef} className="settings-navigation" aria-label={t("Settings categories", "设置分类")}>
      <label className="settings-nav-search">
        <span className="sr-only">{t("Search settings", "搜索设置")}</span>
        <Search size={14} aria-hidden />
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
          {sections.map(({ id, labelEn, labelZh, icon: Icon }) => (
            <NavLink
              key={id}
              to={`/settings/${id}`}
              className={({ isActive }) => (isActive || id === activeSection ? "active" : undefined)}
            >
              <Icon size={15} aria-hidden />
              <span>
                <strong>{t(labelEn, labelZh)}</strong>
              </span>
            </NavLink>
          ))}
        </div>
      ))}
    </nav>
  );
}
