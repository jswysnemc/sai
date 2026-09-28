import type {
  SettingsAppConfigUse,
  SettingsGroupId,
  SettingsGroupMeta,
  SettingsSectionId,
  SettingsSectionMeta
} from "./settings-types";
import { SETTINGS_GROUPS, SETTINGS_SECTIONS } from "./settings-sections";

export { SETTINGS_GROUPS, SETTINGS_SECTIONS } from "./settings-sections";

/** 默认打开的设置 section。 */
export const DEFAULT_SETTINGS_SECTION: SettingsSectionId = "providers";

/** 旧版分区地址到现分区的映射。 */
const LEGACY_SECTION_IDS: Record<string, SettingsSectionId> = {
  plugins: "cli-tools",
  "jev-models": "jev"
};

/**
 * 解析路由 section 参数；旧地址映射到现分区，未知值回退默认 section。
 *
 * @param value 路由参数
 * @returns 合法 SettingsSectionId
 */
export function resolveSettingsSectionId(value: string | undefined | null): SettingsSectionId {
  if (!value) return DEFAULT_SETTINGS_SECTION;
  const legacy = LEGACY_SECTION_IDS[value];
  if (legacy) return legacy;
  const match = SETTINGS_SECTIONS.find((item) => item.id === value);
  return match?.id ?? DEFAULT_SETTINGS_SECTION;
}

/**
 * 按 id 查找 section 元数据。
 *
 * @param id section 标识
 * @returns 元数据；不存在时 undefined
 */
export function getSettingsSection(id: SettingsSectionId): SettingsSectionMeta | undefined {
  return SETTINGS_SECTIONS.find((item) => item.id === id);
}

/**
 * 解析二级子页路由段。
 *
 * 无子页的分区始终返回 undefined；旧子页段按映射表转到新子页；
 * 有子页的分区在段非法或缺失时回落到首个子页，保证 URL 总能归一到显式子页。
 *
 * @param meta 分区元数据
 * @param value 路由中的子页段
 * @returns 合法子页 id；分区无子页时 undefined
 */
export function resolveSettingsSubview(
  meta: SettingsSectionMeta | undefined,
  value: string | undefined | null
): string | undefined {
  const subviews = meta?.subviews;
  if (!subviews || subviews.length === 0) return undefined;
  const mapped = value ? meta?.legacySubviews?.[value] ?? value : value;
  return subviews.find((item) => item.id === mapped)?.id ?? subviews[0].id;
}

/**
 * 判断顶栏是否应展示全局 AppConfig 保存控件。
 *
 * required 面常驻保存；其余分区只在全局草稿有待保存修改时露出，
 * 避免在别处改了配置后切到只读分区就找不到保存入口。
 *
 * @param use 分区对 AppConfig 的参与方式
 * @param dirty 全局草稿是否有待保存修改
 * @returns 需要全局 Save 时 true
 */
export function showsAppConfigSave(use: SettingsAppConfigUse, dirty: boolean): boolean {
  return use === "required" || dirty;
}

/**
 * 按关键字过滤 section（标签、描述、searchKeys）。
 *
 * @param query 用户输入
 * @returns 过滤后的 section 列表
 */
export function filterSettingsSections(query: string): SettingsSectionMeta[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return SETTINGS_SECTIONS;
  return SETTINGS_SECTIONS.filter((item) => {
    const haystacks = [
      item.id,
      item.labelEn,
      item.labelZh,
      item.descriptionEn,
      item.descriptionZh,
      ...item.searchKeys
    ].map((value) => value.toLowerCase());
    return haystacks.some((value) => value.includes(needle));
  });
}

/**
 * 将 section 列表按分组顺序归组。
 *
 * @param sections section 列表
 * @returns 分组后的结构（跳过空组）
 */
export function groupSettingsSections(
  sections: SettingsSectionMeta[]
): Array<{ group: SettingsGroupMeta; sections: SettingsSectionMeta[] }> {
  return SETTINGS_GROUPS.map((group) => ({
    group,
    sections: sections.filter((item) => item.group === group.id)
  })).filter((entry) => entry.sections.length > 0);
}

/**
 * 判断字符串是否为已知分组 id。
 *
 * @param value 候选值
 * @returns 是分组 id 时 true
 */
export function isSettingsGroupId(value: string): value is SettingsGroupId {
  return SETTINGS_GROUPS.some((group) => group.id === value);
}

/**
 * 按点号路径读取配置中的值。
 *
 * @param config 配置对象
 * @param path 点号路径，例如 plugins.web
 * @returns 路径上的值；中途缺失时 undefined
 */
function readConfigPath(config: unknown, path: string): unknown {
  let current: unknown = config;
  for (const segment of path.split(".")) {
    if (!current || typeof current !== "object") return undefined;
    current = (current as Record<string, unknown>)[segment];
  }
  return current;
}

/**
 * 找出草稿相对服务端快照有修改的分区，用于导航上的未保存标记。
 *
 * @param draft 当前草稿
 * @param baseline 服务端快照
 * @returns 有修改的分区 id 集合
 */
export function dirtySettingsSections(draft: unknown, baseline: unknown): Set<SettingsSectionId> {
  const dirty = new Set<SettingsSectionId>();
  if (!draft || !baseline) return dirty;
  for (const section of SETTINGS_SECTIONS) {
    const keys = section.configKeys ?? [];
    // 1. 逐个比较分区声明的配置路径，任一不同即标记
    const changed = keys.some((key) => JSON.stringify(readConfigPath(draft, key)) !== JSON.stringify(readConfigPath(baseline, key)));
    if (changed) dirty.add(section.id);
  }
  return dirty;
}
