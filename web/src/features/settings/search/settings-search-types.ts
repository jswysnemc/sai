import type { SettingsSectionId } from "../settings-types";

/** 字段级搜索条目。 */
export type SettingsSearchEntry = {
  /** 字段锚点，与字段组件的 anchor 一致 */
  anchor: string;
  section: SettingsSectionId;
  /** 字段所在子页；分区无子页时省略 */
  subview?: string;
  /** 对象分区中需要先选中的条目 */
  item?: string;
  /** 对象内部页签，如 Agent 工具权限或 Skills 运行策略 */
  view?: string;
  labelEn: string;
  labelZh: string;
  /** 额外关键词：配置键、同义词、常见说法 */
  keywords?: string[];
};
