import { useState, type ReactNode } from "react";
import { ChevronRight } from "../../../shared/ui/icons";
import { Collapse } from "../../../shared/ui/collapse/collapse";
import { cx } from "./class-names";
import "./settings-panel.css";

type SettingsPanelProps = {
  title: ReactNode;
  /** 一句话说明 */
  description?: ReactNode;
  /** 标题前的图标 */
  icon?: ReactNode;
  /** 标题行右侧操作 */
  actions?: ReactNode;
  /** 低频配置可折叠 */
  collapsible?: boolean;
  /** 可折叠面板的初始展开状态 */
  defaultOpen?: boolean;
  /** 面板锚点标识，供页内跳转 */
  id?: string;
  className?: string;
  children: ReactNode;
};

/**
 * 渲染设置面板：标题、说明与操作在上，字段区在下。
 *
 * 面板本身不带卡片边框，相邻面板以分隔线区分；低频面板可折叠。
 *
 * @param props 标题、说明、图标、操作、折叠设置与内容
 * @returns 设置面板
 */
export function SettingsPanel({
  title,
  description,
  icon,
  actions,
  collapsible = false,
  defaultOpen = true,
  id,
  className,
  children
}: SettingsPanelProps) {
  const [open, setOpen] = useState(defaultOpen);
  const expanded = !collapsible || open;
  const heading = <h3 className="sk-panel-title">{icon}{title}</h3>;
  return (
    <section
      id={id}
      className={cx("sk-panel", collapsible && (open ? "is-open" : "is-collapsed"), className)}
    >
      <header className="sk-panel-head">
        {collapsible ? (
          <button type="button" className="sk-panel-toggle" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
            <ChevronRight size={14} className="sk-panel-chevron" />
            <span className="sk-panel-copy">
              {heading}
              {description && <span className="sk-panel-desc">{description}</span>}
            </span>
          </button>
        ) : (
          <div className="sk-panel-copy">
            {heading}
            {description && <p className="sk-panel-desc">{description}</p>}
          </div>
        )}
        {actions && expanded && <div className="sk-panel-actions">{actions}</div>}
      </header>
      {collapsible
        ? <Collapse open={expanded}><div className="sk-panel-body">{children}</div></Collapse>
        : <div className="sk-panel-body">{children}</div>}
    </section>
  );
}
