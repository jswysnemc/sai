import { useEffect, useMemo, useRef } from "react";
import { Navigate, useLocation, useParams } from "react-router-dom";
import { SettingsNav } from "./shell/settings-nav";
import { SettingsTopbar } from "./shell/settings-topbar";
import { SettingsSectionBody } from "./shell/settings-section-body";
import { SettingsSubnav } from "./shell/settings-subnav";
import { useFieldFocus } from "./shell/use-field-focus";
import { useLeaveGuard } from "./shell/use-leave-guard";
import {
  dirtySettingsSections,
  getSettingsSection,
  resolveSettingsSectionId,
  resolveSettingsSubview
} from "./settings-registry";
import { useSettingsConfig } from "./use-settings-config";
import { useTheme } from "../theme/theme";
import "./kit";
import "./settings-layout.css";
import "./settings-forms.css";
import "./settings-catalog.css";

/**
 * 设置页壳层：顶栏、分组导航、按路由挂载 section 与子页。
 *
 * 壳层持有全局配置草稿，并负责保存快捷键、离开确认与字段定位。
 *
 * @returns 设置页面
 */
export function SettingsPage() {
  const params = useParams<{ sectionId?: string; subview?: string }>();
  const location = useLocation();
  const requested = params.sectionId;
  const section = resolveSettingsSectionId(requested);
  const meta = getSettingsSection(section);
  const subview = resolveSettingsSubview(meta, params.subview);
  const settings = useSettingsConfig();
  const theme = useTheme();
  const pageRef = useRef<HTMLDivElement | null>(null);
  const onExit = useLeaveGuard(settings.dirty, settings.discard);
  const dirtySections = useMemo(
    () => {
      const changed = dirtySettingsSections(settings.config, settings.baseline);
      if (settings.jsonError) changed.add("advanced");
      return changed;
    },
    [settings.baseline, settings.config, settings.jsonError]
  );
  const { dirty, saving, saveConfig } = settings;
  useFieldFocus(`${section}/${subview ?? ""}`);

  useEffect(() => {
    pageRef.current?.scrollTo({ top: 0 });
  }, [section, subview]);

  useEffect(() => {
    /**
     * Ctrl/Cmd+S 保存全局草稿，并阻止浏览器保存网页。
     *
     * @param event 键盘事件
     * @returns 无返回值
     */
    const handleKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "s") return;
      event.preventDefault();
      if (dirty && !saving) void saveConfig().catch(() => undefined);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [dirty, saveConfig, saving]);

  // 1. 归一非法 section 与子页段：有子页的分区始终落在显式子页 URL 上，保留查询参数
  if (!requested || requested !== section || (params.subview ?? undefined) !== subview) {
    const target = subview ? `/settings/${section}/${subview}` : `/settings/${section}`;
    return <Navigate to={`${target}${location.search}`} replace />;
  }

  return (
    <div className="settings-page">
      <SettingsTopbar
        meta={meta}
        subview={subview}
        dirty={settings.dirty}
        saving={settings.saving}
        saveErrorMessage={settings.saveError?.message}
        loaded={Boolean(settings.config)}
        validationError={settings.jsonError}
        onSave={() => void settings.saveConfig().catch(() => undefined)}
        onDiscard={settings.discard}
      />
      <div className="settings-workspace">
        <SettingsNav activeSection={section} config={settings.config} dirtySections={dirtySections} onExit={onExit} />
        <main className="settings-main" ref={pageRef}>
          <div className="settings-main-content" data-layout={meta?.layout ?? "form"}>
            {meta?.subviews && section !== "providers" && (
              <SettingsSubnav sectionId={section} subviews={meta.subviews} />
            )}
            <SettingsSectionBody
              section={section}
              subview={subview}
              settings={settings}
              theme={theme.theme}
              onThemeChange={theme.setTheme}
            />
          </div>
        </main>
      </div>
    </div>
  );
}
