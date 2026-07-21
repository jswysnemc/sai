import { X } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";
import { LightboxImage } from "../../../shared/ui/image-lightbox";
import { Button } from "../../../shared/ui/button/button";
import { MessageActions } from "./message-actions";
import { useI18n } from "../../i18n/use-i18n";
import { UserMessageContent } from "./user-message-content";
import "./user-message-bubble.css";

type UserMessageBubbleProps = {
  content: string;
  timestamp?: string;
  imageUrls?: string[];
  onRetry?: () => void;
  /** 从会话队列移除（仅排队中展示） */
  onRemoveFromQueue?: () => void;
};

const COLLAPSE_HEIGHT = 320;

/**
 * 用户消息气泡，支持 Markdown 渲染、超高折叠、附件放大、复制和重试操作。
 *
 * @param props content 为消息原文，timestamp 为可选时间，imageUrls 为附件图片地址，onRetry 为可选的重试回调
 * @returns 右对齐的用户消息气泡
 */
export function UserMessageBubble({ content, timestamp, imageUrls, onRetry, onRemoveFromQueue }: UserMessageBubbleProps) {
  const { t } = useI18n();
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const [collapsible, setCollapsible] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [removing, setRemoving] = useState(false);

  // 1. 内容变化时测量高度决定是否需要折叠
  useLayoutEffect(() => {
    const node = bodyRef.current;
    if (!node) return;
    setCollapsible(node.scrollHeight > COLLAPSE_HEIGHT + 40);
  }, [content]);

  const collapsed = collapsible && !expanded;

  /**
   * 从会话队列移除本条排队消息。
   *
   * @returns 无
   */
  const removeFromQueue = () => {
    if (!onRemoveFromQueue || removing) return;
    setRemoving(true);
    try {
      onRemoveFromQueue();
    } finally {
      // 事件到达后父级会卸载本组件；失败时允许再次点击
      setRemoving(false);
    }
  };

  return (
    <article className={`message user-message${onRemoveFromQueue ? " is-queued" : ""}`}>
      <div className="user-message-stack">
        <div className="user-bubble">
          {onRemoveFromQueue && (
            <button
              type="button"
              className="user-queue-remove"
              onClick={removeFromQueue}
              disabled={removing}
              aria-label={t("Remove from queue", "从队列移除")}
              title={t("Remove from queue", "从队列移除")}
            >
              <X size={12} />
            </button>
          )}
          {onRemoveFromQueue && (
            <div className="user-queue-badge" role="status">
              {t("Queued", "排队中")}
            </div>
          )}
          {imageUrls && imageUrls.length > 0 && (
            <div className="user-attachments">
              {imageUrls.map((url, index) => (
                <LightboxImage className="user-attachment" src={url} alt={t(`User attachment ${index + 1}`, `用户附件 ${index + 1}`)} key={`${index}-${url.slice(-24)}`} />
              ))}
            </div>
          )}
          <div ref={bodyRef} className={`message-content user-markdown${collapsed ? " collapsed" : ""}`}>
            <UserMessageContent content={content} />
          </div>
          {collapsible && (
            <Button className="bubble-expand" onClick={() => setExpanded((value) => !value)}>
              {expanded ? t("Collapse", "收起") : t("Show more", "显示更多")}
            </Button>
          )}
        </div>
        <MessageActions className="user-message-actions" text={content} timestamp={timestamp} onRetry={onRetry} />
      </div>
    </article>
  );
}
