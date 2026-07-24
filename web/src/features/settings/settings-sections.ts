/**
 * 兼容旧导入路径：设置 section 注册表已迁至 settings-registry。
 */
export {
  DEFAULT_SETTINGS_SECTION,
  SETTINGS_GROUPS,
  SETTINGS_SECTIONS,
  filterSettingsSections,
  getSettingsSection,
  groupSettingsSections,
  resolveSettingsSectionId,
  showsGlobalAppConfigSave
} from "./settings-registry";
export type { SettingsSectionMeta } from "./settings-types";
