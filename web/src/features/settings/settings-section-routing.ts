import type { SettingsSectionId } from "./settings-types";

/**
 * 需要全局 AppConfig 才能渲染的分区。
 *
 * 分区体按此分流到两个渲染函数：登记在此的走 AppConfig 分支，
 * 其余走独立分支。新增分区必须落到其中一侧，否则界面渲染为空白。
 */
export const APP_CONFIG_SECTION_IDS = [
  "providers",
  "agents",
  "cli-tools",
  "web-search",
  "runtime",
  "prompts",
  "git",
  "hooks",
  "gateways",
  "advanced"
] as const satisfies readonly SettingsSectionId[];

/** 自带数据加载、不依赖全局 AppConfig 的分区。 */
export const STANDALONE_SECTION_IDS = [
  "appearance",
  "skills",
  "memory",
  "mcp",
  "ssh",
  "usage",
  "session-data"
] as const satisfies readonly SettingsSectionId[];

/**
 * 判断分区是否由 AppConfig 渲染分支承载。
 *
 * @param section 分区标识
 * @returns 属于 AppConfig 分支时返回 true
 */
export function isAppConfigSection(section: SettingsSectionId): boolean {
  return (APP_CONFIG_SECTION_IDS as readonly string[]).includes(section);
}
