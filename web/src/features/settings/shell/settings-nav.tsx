import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { Link, NavLink, useNavigate } from "react-router-dom";
import { ArrowLeft, Search, X } from "../../../shared/ui/icons";
import { filterSettingsSections, groupSettingsSections } from "../settings-registry";
import { searchEntryHref, searchSettingsFields } from "../search/settings-search";
import type { SettingsSectionId } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";
import type { AppConfig } from "../../../api/contracts";
import { SETTINGS_SEARCH_INDEX } from "../search/settings-search-index";
import { buildCapabilitySearchEntries } from "../search/entries-agent-capabilities";
import { buildCliToolSearchEntries } from "../search/entries-cli-tools";

type SettingsNavProps = {
  activeSection: SettingsSectionId;
  config?: AppConfig | null;
  /** 有未保存修改的分区 */
  dirtySections: ReadonlySet<SettingsSectionId>;
  /** 返回主界面链接的点击处理，用于未保存修改确认 */
  onExit: (event: MouseEvent<HTMLAnchorElement>, to: string) => void;
};

/**
 * 渲染设置导航：返回入口、字段级搜索、分组分区与未保存标记。
 *
 * 搜索同时匹配分区与字段：字段结果点击后跳到所在分区与子页并高亮字段。
 *
 * @param props 当前分区、未保存分区与离开处理
 * @returns 侧栏导航；窄屏下为横向分类栏
 */
export function SettingsNav({ activeSection, config, dirtySections, onExit }: SettingsNavProps) {
  const { t, locale } = useI18n();
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const navigationRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const media = window.matchMedia("(width < 48rem)");
    let frame = 0;
    /**
     * 在横向分类栏中显示当前分类，同时清空已隐藏搜索框中的筛选条件。
     *
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

  // 1. 分区按关键字过滤后归组；字段结果单独排序
  const grouped = useMemo(() => groupSettingsSections(filterSettingsSections(query)), [query]);
  const searchIndex = useMemo(() => [...SETTINGS_SEARCH_INDEX, ...buildCliToolSearchEntries(config), ...buildCapabilitySearchEntries(config)], [config]);
  const fieldHits = useMemo(() => searchSettingsFields(query, locale, searchIndex), [locale, query, searchIndex]);
  const searching = query.trim().length > 0;

  /**
   * 回车跳到第一个字段结果；没有字段结果时打开第一个分区。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handleSearchKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      setQuery("");
      return;
    }
    if (event.key !== "Enter") return;
    const firstField = fieldHits[0];
    if (firstField) {
      navigate(searchEntryHref(firstField.entry));
      return;
    }
    const firstSection = grouped[0]?.sections[0];
    if (firstSection) navigate(`/settings/${firstSection.id}`);
  };

  return (
    <nav ref={navigationRef} className="settings-navigation" aria-label={t("Settings categories", "设置分类")}>
      <Link to="/" className="settings-back" aria-label={t("Back to workspace", "返回主界面")} onClick={(event) => onExit(event, "/")}>
        <ArrowLeft size={16} />
        <span>{t("Back to workspace", "返回主界面")}</span>
      </Link>
      <label className="settings-nav-search">
        <Search size={14} aria-hidden />
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={handleSearchKeyDown}
          placeholder={t("Search settings", "搜索设置项")}
          aria-label={t("Search settings", "搜索设置项")}
        />
        {searching && (
          <button type="button" className="settings-nav-search-clear" onClick={() => setQuery("")} aria-label={t("Clear search", "清空搜索")}>
            <X size={12} />
          </button>
        )}
      </label>
      {searching && fieldHits.length > 0 && (
        <div className="settings-nav-group settings-nav-fields">
          <div className="settings-nav-group-label">{t("Settings", "设置项")}</div>
          {fieldHits.map((hit) => (
            <Link key={`${hit.entry.section}:${hit.entry.item ?? ""}:${hit.entry.anchor}`} to={searchEntryHref(hit.entry)} className="settings-nav-field">
              <strong>{hit.label}</strong>
              <small>{hit.location}</small>
            </Link>
          ))}
        </div>
      )}
      {searching && grouped.length === 0 && fieldHits.length === 0 && (
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
              <Icon size={16} aria-hidden />
              <span>
                <strong>{t(labelEn, labelZh)}</strong>
              </span>
              {dirtySections.has(id) && (
                <span className="settings-nav-dirty" role="img" title={t("Unsaved changes", "有未保存的修改")} aria-label={t("Unsaved changes", "有未保存的修改")} />
              )}
            </NavLink>
          ))}
        </div>
      ))}
    </nav>
  );
}
