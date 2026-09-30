import { useId, useState, type ReactNode } from "react";
import { ChevronRight } from "../../../shared/ui/icons";
import { Collapse } from "../../../shared/ui/collapse/collapse";
import "./disclosure-item.css";

type DisclosureItemProps = {
  /** 条目名称 */
  title: ReactNode;
  /** 名称右侧的一句说明或状态 */
  meta?: ReactNode;
  /** 展开后的正文；为空时条目不可展开 */
  children?: ReactNode;
  /** 初始是否展开 */
  defaultOpen?: boolean;
};

/**
 * 【工具结果】【可展开条目】单行标题 + 按需展开正文，用于 Jev 注入片段与 load 结果。
 *
 * 默认折叠，只露出名称与一句说明；点击整行展开正文，避免长文档铺满对话。
 *
 * @param props 标题、说明、正文与初始状态
 * @returns 可展开条目
 */
export function DisclosureItem({ title, meta, children, defaultOpen = false }: DisclosureItemProps) {
  const [open, setOpen] = useState(defaultOpen);
  const bodyId = useId();
  const expandable = children !== undefined && children !== null && children !== false;
  const head = (
    <>
      <ChevronRight size={12} className={`disclosure-item-chevron${open ? " is-open" : ""}${expandable ? "" : " is-hidden"}`} aria-hidden="true" />
      <strong className="disclosure-item-title">{title}</strong>
      {meta ? <span className="disclosure-item-meta">{meta}</span> : null}
    </>
  );
  return (
    <li className="disclosure-item">
      {expandable ? (
        <button
          type="button"
          className="disclosure-item-head"
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={() => setOpen((value) => !value)}
        >
          {head}
        </button>
      ) : (
        <div className="disclosure-item-head">{head}</div>
      )}
      {expandable ? (
        <Collapse open={open}>
          <div id={bodyId} className="disclosure-item-body">{children}</div>
        </Collapse>
      ) : null}
    </li>
  );
}
