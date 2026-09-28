import type { ReactNode } from "react";
import { cx } from "./class-names";
import "./master-detail.css";

type MasterDetailProps = {
  /** 左侧对象列表，通常为 ObjectList */
  list: ReactNode;
  className?: string;
  children: ReactNode;
};

/**
 * 渲染对象分区的两栏布局：左侧列表吸顶，右侧详情随页面滚动。
 *
 * 外层是容器查询宿主，容器变窄时列表收起为详情上方的选择器。
 *
 * @param props 列表节点与详情内容
 * @returns 对象分区布局
 */
export function MasterDetail({ list, className, children }: MasterDetailProps) {
  return (
    <div className={cx("sk-md-host", className)}>
      <div className="sk-master-detail">
        <aside className="sk-master">{list}</aside>
        <section className="sk-detail">{children}</section>
      </div>
    </div>
  );
}
