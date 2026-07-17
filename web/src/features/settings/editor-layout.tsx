import type { ReactNode } from "react";

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
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
};

/**
 * 渲染分组标题加分隔线的表单分组，替代嵌套圆角卡片。
 *
 * @param props 分组标题、说明、操作节点和分组内容
 * @returns 表单分组
 */
export function SettingsGroup({ title, description, actions, children }: SettingsGroupProps) {
  return (
    <section className="settings-group">
      <div className="settings-group-head">
        <div><h3>{title}</h3>{description && <p>{description}</p>}</div>
        {actions}
      </div>
      {children}
    </section>
  );
}
