import type { ReactNode } from "react";
import "./empty-guide.css";

/** 空状态中的快速创建模板。 */
export type EmptyTemplate = {
  id: string;
  label: string;
  description?: string;
  onSelect: () => void;
};

type EmptyGuideProps = {
  icon?: ReactNode;
  title: string;
  description?: ReactNode;
  /** 主要操作，例如「新增」按钮 */
  action?: ReactNode;
  /** 常用模板，点击即创建 */
  templates?: EmptyTemplate[];
};

/**
 * 渲染紧凑空状态：一行说明加操作，可附常用模板，不占满整个视窗。
 *
 * @param props 图标、标题、说明、主要操作与模板
 * @returns 空状态引导条
 */
export function EmptyGuide({ icon, title, description, action, templates }: EmptyGuideProps) {
  return (
    <div className="sk-empty">
      <div className="sk-empty-head">
        {icon && <span className="sk-empty-icon">{icon}</span>}
        <div className="sk-empty-copy">
          <strong>{title}</strong>
          {description && <p>{description}</p>}
        </div>
        {action && <div className="sk-empty-action">{action}</div>}
      </div>
      {templates && templates.length > 0 && (
        <div className="sk-empty-templates">
          {templates.map((template) => (
            <button type="button" className="sk-empty-template" key={template.id} onClick={template.onSelect}>
              <strong>{template.label}</strong>
              {template.description && <small>{template.description}</small>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
