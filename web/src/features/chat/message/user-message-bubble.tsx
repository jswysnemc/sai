import { useLayoutEffect, useRef, useState } from "react";
import { LightboxImage } from "../../../shared/ui/image-lightbox";
import { MarkdownRenderer } from "../markdown-renderer";
import { MessageActions } from "./message-actions";
import { useI18n } from "../../i18n/use-i18n";

type UserMessageBubbleProps = {
  content: string;
  timestamp?: string;
  imageUrls?: string[];
  onRetry?: () => void;
};

const COLLAPSE_HEIGHT = 320;

/**
 * 用户消息气泡，支持 Markdown 渲染、超高折叠、附件放大、复制和重试操作。
 *
 * @param props content 为消息原文，timestamp 为可选时间，imageUrls 为附件图片地址，onRetry 为可选的重试回调
 * @returns 右对齐的用户消息气泡
 */
export function UserMessageBubble({ content, timestamp, imageUrls, onRetry }: UserMessageBubbleProps) {
  const { t } = useI18n();
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const [collapsible, setCollapsible] = useState(false);
  const [expanded, setExpanded] = useState(false);

  // 1. 内容变化时测量高度决定是否需要折叠
  useLayoutEffect(() => {
    const node = bodyRef.current;
    if (!node) return;
    setCollapsible(node.scrollHeight > COLLAPSE_HEIGHT + 40);
  }, [content]);

  const collapsed = collapsible && !expanded;
  return (
    <article className="message user-message">
      <div className="user-message-stack">
        {imageUrls && imageUrls.length > 0 && (
          <div className="user-attachments">
            {imageUrls.map((url, index) => (
              <LightboxImage className="user-attachment" src={url} alt={t(`User attachment ${index + 1}`, `用户附件 ${index + 1}`)} key={`${index}-${url.slice(-24)}`} />
            ))}
          </div>
        )}
        <div className="user-bubble">
          <div ref={bodyRef} className={`message-content user-markdown${collapsed ? " collapsed" : ""}`}>
            <MarkdownRenderer source={content} />
          </div>
          {collapsible && (
            <button type="button" className="bubble-expand" onClick={() => setExpanded((value) => !value)}>
              {expanded ? t("Collapse", "收起") : t("Show more", "显示更多")}
            </button>
          )}
          <MessageActions text={content} timestamp={timestamp} onRetry={onRetry} />
        </div>
      </div>
    </article>
  );
}
