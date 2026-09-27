import { createLucideIcon } from "lucide-react";
import { withIconDefaults } from "../with-icon-defaults";
import { DIAMOND_CHECK, WORKBENCH_FILES, WORKBENCH_REVIEW } from "./custom-icon-nodes";

// 自绘图标经 createLucideIcon 生成，与 Lucide 图标共享类名、描边与尺寸阶梯。

/** Jev 判断与决策。 */
export const DiamondCheck = withIconDefaults(createLucideIcon("DiamondCheck", DIAMOND_CHECK));
/** 工作台文件入口。 */
export const WorkbenchFiles = withIconDefaults(createLucideIcon("WorkbenchFiles", WORKBENCH_FILES));
/** 工作台审阅入口。 */
export const WorkbenchReview = withIconDefaults(createLucideIcon("WorkbenchReview", WORKBENCH_REVIEW));
