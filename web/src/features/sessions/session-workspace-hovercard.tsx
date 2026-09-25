import { useRef, useState, type FocusEvent, type MouseEvent, type ReactNode } from "react";
import "./session-row.css";

type SessionWorkspaceHovercardProps = {
  name: string;
  path: string;
  children: ReactNode;
};

const HOVER_DELAY_MS = 420;
const CARD_WIDTH = 240;
const CARD_HEIGHT = 52;

/**
 * 悬停时在行外侧展示工作区名称和路径。
 *
 * 稍作延迟，并优先放到行的右侧，避免盖住下一行、右键菜单和行内按钮。
 *
 * @param props name 为工作区名称，path 为目录，children 为会话或工作区行
 * @returns 带延迟悬停卡片的行
 */
export function SessionWorkspaceHovercard({ name, path, children }: SessionWorkspaceHovercardProps) {
  const timer = useRef<number | null>(null);
  const [box, setBox] = useState<{ top: number; left: number } | null>(null);

  /**
   * 取消尚未出现的卡片。
   */
  const clearTimer = () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
  };

  /**
   * 把卡片放到行的右侧；右侧放不下时改到行的上方。
   *
   * @param rect 行的视口矩形
   * @returns 卡片左上角
   */
  const place = (rect: DOMRect) => {
    const right = rect.right + 8;
    if (right + CARD_WIDTH <= window.innerWidth - 8) {
      return { left: right, top: Math.max(8, Math.min(rect.top, window.innerHeight - CARD_HEIGHT - 8)) };
    }
    return {
      left: Math.max(8, Math.min(rect.left, window.innerWidth - CARD_WIDTH - 8)),
      top: Math.max(8, rect.top - CARD_HEIGHT - 6)
    };
  };

  /**
   * 停留一会儿后再显示，快速划过不闪卡片。
   *
   * @param event 进入或聚焦事件
   */
  const schedule = (event: MouseEvent<HTMLDivElement> | FocusEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    clearTimer();
    timer.current = window.setTimeout(() => setBox(place(rect)), HOVER_DELAY_MS);
  };

  /**
   * 离开、右键时收起卡片，把位置让给菜单。
   */
  const hide = () => {
    clearTimer();
    setBox(null);
  };

  return (
    <div
      className="session-workspace-hover"
      onMouseEnter={schedule}
      onMouseLeave={hide}
      onFocus={schedule}
      onBlur={hide}
      onContextMenu={hide}
    >
      {children}
      {box && name && (
        <div className="session-workspace-card" role="tooltip" style={{ top: box.top, left: box.left, width: CARD_WIDTH }}>
          <strong>{name}</strong>
          <small>{path}</small>
        </div>
      )}
    </div>
  );
}
