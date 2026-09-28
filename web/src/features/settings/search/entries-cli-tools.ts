import type { AppConfig } from "../../../api/contracts";
import { getCliToolCatalogEntry } from "../cli-tools/cli-tool-catalog";
import { fieldLabel } from "../structured-field-metadata";
import type { SettingsSearchEntry } from "./settings-search-types";

/**
 * 【Web 设置】【工具索引】仅用配置键生成字段索引，不收集字段值或凭据。
 * @param config 当前配置，用于确认实际存在的工具与字段
 * @returns 可定位具体工具的字段条目
 */
export function buildCliToolSearchEntries(config: AppConfig | null | undefined): SettingsSearchEntry[] {
  return Object.entries(config?.plugins ?? {}).flatMap(([id, fields]) => {
    if (id === "web") return [];
    const tool = getCliToolCatalogEntry(id);
    return fieldPaths(fields).map((path) => {
      const name = path.split(".").at(-1) ?? path;
      return {
        anchor: `cli-tools.${id}.${path}`, section: "cli-tools", item: id,
        labelEn: `${tool.labelEn} · ${fieldLabel(name, (en) => en)}`,
        labelZh: `${tool.labelZh} · ${fieldLabel(name, (_en, zh) => zh)}`,
        keywords: [`plugins.${id}.${path}`, tool.labelEn, tool.labelZh]
      };
    });
  });
}

/**
 * 提取结构化控件实际渲染的叶字段路径。
 * @param fields 当前配置对象
 * @param prefix 当前嵌套路径
 * @returns 点号分隔的叶字段路径，数组作为单个可编辑字段
 */
function fieldPaths(fields: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(fields).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return value && typeof value === "object" && !Array.isArray(value) ? fieldPaths(value as Record<string, unknown>, path) : [path];
  });
}
