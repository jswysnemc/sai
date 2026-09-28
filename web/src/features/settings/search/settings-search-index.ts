import { WEB_SEARCH_ENTRIES } from "./entries-web-search";
import { MODEL_SERVICE_SEARCH_ENTRIES } from "./entries-model-services";
import { GIT_SEARCH_ENTRIES } from "./entries-git";
import { RUNTIME_SEARCH_ENTRIES } from "./entries-runtime";
import { PERSONALIZATION_SEARCH_ENTRIES } from "./entries-personalization";
import type { SettingsSearchEntry } from "./settings-search-types";

export type { SettingsSearchEntry } from "./settings-search-types";

/**
 * 【Web 设置】【字段搜索】字段索引。
 *
 * 每个分区的条目放在同目录 entries-*.ts 中，迁移分区时同步登记；
 * anchor 必须与字段组件的 anchor 属性一致，settings-search.test.ts
 * 校验分区与子页均已注册、锚点不重复。
 */
export const SETTINGS_SEARCH_INDEX: readonly SettingsSearchEntry[] = [
  ...GIT_SEARCH_ENTRIES,
  ...RUNTIME_SEARCH_ENTRIES,
  ...PERSONALIZATION_SEARCH_ENTRIES,
  ...WEB_SEARCH_ENTRIES,
  ...MODEL_SERVICE_SEARCH_ENTRIES
];
