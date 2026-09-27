import { useState, type ReactNode } from "react";
import { ChevronRight } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { Collapse } from "../../shared/ui/collapse/collapse";

type EditorHeaderProps = {
  kicker: string;
  title: string;
  description?: string;
  actions?: ReactNode;
};

/**
 * 渲染编辑区顶部标题行，操作按钮固定在右侧且危险按钮排最右。
 *
 * @param props 眉标、标题、说明和操作节点
 * @returns 编辑区标题行
 */
export function EditorHeader({ kicker, title, description, actions }: EditorHeaderProps) {
  return (
    <header className="editor-header">
      <div className="editor-header-copy">
        <span className="settings-kicker">{kicker}</span>
        <h2>{title}</h2>
        {description && <p>{description}</p>}
      </div>
      {actions && <div className="editor-header-actions">{actions}</div>}
    </header>
  );
}

type SettingsGroupProps = {
  title: string;
  icon?: ReactNode;
  description?: string;
  actions?: ReactNode;
  /** 低频配置可折叠，标题行变为展开按钮 */
  collapsible?: boolean;
  /** 可折叠分组的初始展开状态 */
  defaultOpen?: boolean;
  children: ReactNode;
};

/**
 * 渲染分组：标题在卡片外，字段收进同一张卡片；低频分组可折叠。
 *
 * @param props 分组标题、可选图标、说明、操作节点、折叠设置和分组内容
 * @returns 表单分组
 */
export function SettingsGroup({ title, icon, description, actions, collapsible = false, defaultOpen = true, children }: SettingsGroupProps) {
  const [open, setOpen] = useState(defaultOpen);
  const expanded = !collapsible || open;
  return (
    <section className={`settings-group${collapsible ? " is-collapsible" : ""}`}>
      <div className="settings-group-head">
        {collapsible ? (
          <Button variant="ghost" className="settings-group-toggle" aria-expanded={open} onClick={() => setOpen((value) => !value)}>
            <span className="icon-label">
              <ChevronRight size={14} className={open ? "settings-group-chevron is-open" : "settings-group-chevron"} aria-hidden />
              <span className="settings-group-toggle-copy">
                <span className="settings-group-title">{icon}<span className="settings-group-heading" role="heading" aria-level={3}>{title}</span></span>
                {description && <span className="settings-group-description">{description}</span>}
              </span>
            </span>
          </Button>
        ) : (
          <div>
            <div className="settings-group-title">{icon}<h3>{title}</h3></div>
            {description && <p>{description}</p>}
          </div>
        )}
        {actions}
      </div>
      {collapsible
        ? <Collapse open={expanded}><div className="settings-group-card">{children}</div></Collapse>
        : <div className="settings-group-card">{children}</div>}
    </section>
  );
}
