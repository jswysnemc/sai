import { parseAutomaticInput } from "./parse-automatic-input";
import "./automatic-input-part.css";

/**
 * 渲染 Sai 主动提交给模型的自动输入。
 *
 * 左侧圆点占用与工具行相同的 20px 图标列，标题与工具摘要齐平；
 * 命令、状态、说明各占一行，不再被 Markdown 收成一段后从词中间折行。
 *
 * @param props 自动输入文本
 * @returns 带蓝色圆点的自动消息部件
 */
export function AutomaticInputPart({ content }: { content: string }) {
  const model = parseAutomaticInput(content);
  return (
    <div className="automatic-input-part">
      <span className="automatic-input-dot" aria-hidden="true" />
      <div className="automatic-input-content">
        {model.title && <p className="automatic-input-title">{model.title}</p>}
        {model.notices.map((notice, index) => (
          <div key={`${notice.fields[0]?.value ?? notice.leftover}:${index}`} className="automatic-input-notice">
            {notice.fields.length > 0 && (
              <dl className="automatic-input-fields">
                {notice.fields.map((field) => (
                  <div key={field.label} className="automatic-input-field">
                    <dt>{field.label}</dt>
                    <dd>{field.value}</dd>
                  </div>
                ))}
              </dl>
            )}
            {notice.leftover && <p className="automatic-input-leftover">{notice.leftover}</p>}
          </div>
        ))}
      </div>
    </div>
  );
}
