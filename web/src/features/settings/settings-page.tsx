import { useEffect, useMemo, useRef } from "react";
import { Navigate, useLocation, useParams } from "react-router-dom";
import { SettingsNav } from "./shell/settings-nav";
import { SettingsTopbar } from "./shell/settings-topbar";
import { SettingsSectionBody } from "./shell/settings-section-body";
import { SettingsSubnav } from "./shell/settings-subnav";
import { useFieldFocus } from "./shell/use-field-focus";
import { SettingsDraftProvider, useSettingsDrafts } from "./shell/settings-draft-context";
import { useLeaveGuard } from "./shell/use-leave-guard";
import {
  dirtySettingsSections,
  getSettingsSection,
  resolveSettingsSectionId,
  resolveSettingsSubview
} from "./settings-registry";
import { useSettingsConfig } from "./use-settings-config";
import { useTheme } from "../theme/theme";
import { useI18n } from "../i18n/use-i18n";
import { InlineNotice } from "./kit";
import "./kit";
import "./settings-layout.css";

/**
 * 设置页壳层：顶栏、分组导航、按路由挂载 section 与子页。
 *
 * 壳层持有全局配置草稿，并负责保存快捷键、离开确认与字段定位。
 *
 * @returns 设置页面
 */
export function SettingsPage() {
  return <SettingsDraftProvider><SettingsWorkspace /></SettingsDraftProvider>;
}

/** 组合页面、草稿与导航；无参数，返回设置工作区。 */
function SettingsWorkspace() {
  const params = useParams<{ sectionId?: string; subview?: string }>();
  const location = useLocation();
  const requested = params.sectionId;
  const section = resolveSettingsSectionId(requested);
  const meta = getSettingsSection(section);
  const subview = resolveSettingsSubview(meta, params.subview);
  const settings = useSettingsConfig();
  const theme = useTheme();
  const { t } = useI18n();
  const drafts = useSettingsDrafts();
  const pageRef = useRef<HTMLDivElement | null>(null);
  useLeaveGuard(settings.dirty, settings.discard);
  const dirtySections = useMemo(
    () => {
      const changed = dirtySettingsSections(settings.config, settings.baseline);
      if (settings.jsonError) changed.add("advanced");
      for (const draft of drafts) if (draft.dirty) changed.add(resolveSettingsSectionId(draft.scope.split("/").at(-1)));
      return changed;
    },
    [settings.baseline, settings.config, settings.jsonError, drafts]
  );
  const { dirty, saving, saveConfig } = settings;
  const missingField = useFieldFocus(`${section}/${subview ?? ""}`);

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
      const local = drafts.find((draft) => draft.scope === location.pathname && draft.dirty);
      if (local) { if (!local.saving) void local.save?.().catch(() => undefined); }
      else if (dirty && !saving) void saveConfig().catch(() => undefined);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [dirty, saveConfig, saving, drafts, location.pathname]);

  // 1. 归一非法 section 与子页段：有子页的分区始终落在显式子页 URL 上，保留查询参数
  if (!requested || requested !== section || (params.subview ?? undefined) !== subview) {
    const target = subview ? `/settings/${section}/${subview}` : `/settings/${section}`;
    const query = new URLSearchParams(location.search);
    if (section === "usage" && ["providers", "models", "sessions"].includes(params.subview ?? "")) query.set("view", params.subview!);
    return <Navigate to={`${target}${query.size ? `?${query}` : ""}`} replace />;
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
        <SettingsNav activeSection={section} config={settings.config} dirtySections={dirtySections} />
        <main className="settings-main" ref={pageRef}>
          <div className="settings-main-content" data-layout={meta?.layout ?? "form"}>
            {missingField && <InlineNotice>{missingField.startsWith("runtime.agent.acp.")
              ? t("This field is available when an external conversation engine is selected. Choose the engine above to configure it.", "选择外部对话内核后才会显示此字段，请在对话内核选项中选择对应内核。")
              : t("This field is unavailable for the current selection. Select the matching object or enable its related option, then search again.", "当前选择未显示此字段。请选择对应对象或启用相关选项后重新搜索。")}</InlineNotice>}
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
