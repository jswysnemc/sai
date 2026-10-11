import type { SettingsSearchEntry } from "./settings-search-types";

/** 桌面操作字段索引。 */
export const COMPUTER_USE_ENTRIES: SettingsSearchEntry[] = [
  {
    anchor: "computer-use.enabled",
    section: "computer-use",
    labelEn: "Enable Computer Use",
    labelZh: "启用桌面操作",
    keywords: ["computer_use.enabled", "cua", "desktop"]
  },
  {
    anchor: "computer-use.driver_path",
    section: "computer-use",
    labelEn: "Driver path",
    labelZh: "驱动路径",
    keywords: ["computer_use.driver_path", "cua-driver"]
  },
  {
    anchor: "computer-use.timeout_ms",
    section: "computer-use",
    labelEn: "Call timeout",
    labelZh: "调用超时",
    keywords: ["computer_use.timeout_ms"]
  }
];
