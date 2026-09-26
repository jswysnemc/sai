import type { ReactNode } from "react";

type WorkbenchIconProps = {
  /** 显示边长，路径始终画在同一 16 视口里 */
  size?: number;
};

/**
 * 工作台图标共用的描边视口。
 *
 * 图形都落在 3.5–12.5 的方框内，视觉重心在 (8, 8)，不画外层封闭框。
 *
 * @param props 边长与路径
 * @returns 16 视口的描边图标
 */
function WorkbenchIcon({ size = 15, children }: WorkbenchIconProps & { children: ReactNode }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <g stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        {children}
      </g>
    </svg>
  );
}

/**
 * 文件入口：目录分支，没有文件夹外框。
 *
 * @param props 显示边长
 * @returns 文件图标
 */
export function IconFiles({ size }: WorkbenchIconProps) {
  return (
    <WorkbenchIcon size={size}>
      <path d="M3.5 4.75h3.25M3.5 8h2.5M3.5 11.25h3.25M9.25 4.75v6.5M9.25 8h3.25M9.25 11.25h3.25" />
    </WorkbenchIcon>
  );
}

/**
 * 审阅入口：左右两列差异，没有文档外框。
 *
 * @param props 显示边长
 * @returns 审阅图标
 */
export function IconReview({ size }: WorkbenchIconProps) {
  return (
    <WorkbenchIcon size={size}>
      <path d="M3.5 4.75h3.5M9.25 4.75h3.25M3.5 8h2.25M8.5 8h4M3.5 11.25h4M9.75 11.25h2.75" />
    </WorkbenchIcon>
  );
}
