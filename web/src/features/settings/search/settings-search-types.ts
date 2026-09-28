import type { SettingsSectionId } from "../settings-types";

/** 字段级搜索条目。 */
export type SettingsSearchEntry = {
  /** 字段锚点，与字段组件的 anchor 一致 */
  anchor: string;
  section: SettingsSectionId;
  /** 字段所在子页；分区无子页时省略 */
  subview?: string;
  labelEn: string;
  labelZh: string;
  /** 额外关键词：配置键、同义词、常见说法 */
  keywords?: string[];
};
