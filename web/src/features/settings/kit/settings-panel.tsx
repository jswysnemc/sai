import { useState, type ReactNode } from "react";
import { ChevronRight } from "../../../shared/ui/icons";
import { Collapse } from "../../../shared/ui/collapse/collapse";
import { cx } from "./class-names";
import { SettingsGroup } from "./settings-group";
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
 * 渲染设置分区：13px 次级标签在上，分组卡片包住字段行。
 *
 * 低频面板可折叠；卡片本身不裁切溢出，避免行内下拉被挡住。
 *
 * @param props 标题、说明、图标、操作、折叠设置与内容
 * @returns 设置分区
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
              {description ? <span className="sk-panel-desc">{description}</span> : null}
            </span>
          </button>
        ) : (
          <div className="sk-panel-copy">
            {heading}
            {description ? <p className="sk-panel-desc">{description}</p> : null}
          </div>
        )}
        {actions && expanded ? <div className="sk-panel-actions">{actions}</div> : null}
      </header>
      {collapsible
        ? <Collapse open={expanded}><SettingsGroup>{children}</SettingsGroup></Collapse>
        : <SettingsGroup>{children}</SettingsGroup>}
    </section>
  );
}
