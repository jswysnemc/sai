import { MarkdownRenderer } from "../markdown-renderer";
import "./automatic-input-part.css";

/**
 * 渲染 Sai 主动提交给模型的自动输入。
 *
 * @param props 自动输入文本
 * @returns 带蓝色圆点的自动消息部件
 */
export function AutomaticInputPart({ content }: { content: string }) {
  return (
    <div className="automatic-input-part">
      <span className="automatic-input-dot" aria-hidden="true" />
      <div className="automatic-input-content">
        <MarkdownRenderer source={content} />
      </div>
    </div>
  );
}
