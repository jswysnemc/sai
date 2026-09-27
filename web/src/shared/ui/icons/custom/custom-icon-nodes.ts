import type { IconNode } from "lucide-react";

// 自绘图标沿用 Lucide 规范：24 视口、2px 安全边距、圆头圆角、只描边不填充，
// 几何中心落在 (12, 12)，与 Lucide 图标混排时视觉重量一致。

/** Lucide Diamond 的外轮廓，DiamondPlus / DiamondMinus 同款。 */
const DIAMOND_OUTLINE =
  "M2.7 10.3a2.41 2.41 0 0 0 0 3.41l7.59 7.59a2.41 2.41 0 0 0 3.41 0l7.59-7.59a2.41 2.41 0 0 0 0-3.41l-7.59-7.59a2.41 2.41 0 0 0-3.41 0Z";

/** Jev 判断：菱形判定框内一个对勾，补齐 Lucide Diamond 系列缺少的确认态。 */
export const DIAMOND_CHECK: IconNode = [
  ["path", { d: DIAMOND_OUTLINE, key: "diamond" }],
  ["path", { d: "m8.75 12 2.25 2.25 4.25-4.5", key: "check" }]
];

/** 工作台文件入口：左侧条目加右侧分支，不画文件夹外框，16px 下仍清晰。 */
export const WORKBENCH_FILES: IconNode = [
  ["path", { d: "M5 7h5", key: "item-top" }],
  ["path", { d: "M5 12h4", key: "item-middle" }],
  ["path", { d: "M5 17h5", key: "item-bottom" }],
  ["path", { d: "M14 7v10", key: "trunk" }],
  ["path", { d: "M14 12h5", key: "branch-middle" }],
  ["path", { d: "M14 17h5", key: "branch-bottom" }]
];

/** 工作台审阅入口：左右两列长短不一的行，表达逐行对照，不画文档外框。 */
export const WORKBENCH_REVIEW: IconNode = [
  ["path", { d: "M5 7h5", key: "left-top" }],
  ["path", { d: "M14 7h5", key: "right-top" }],
  ["path", { d: "M5 12h3", key: "left-middle" }],
  ["path", { d: "M13 12h6", key: "right-middle" }],
  ["path", { d: "M5 17h6", key: "left-bottom" }],
  ["path", { d: "M15 17h4", key: "right-bottom" }]
];
