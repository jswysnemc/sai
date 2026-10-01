import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { useI18n } from "../i18n/use-i18n";
import { isNativePasteShortcut, keyMessage, mouseMessage, type ViewportGeometry } from "./browser-input";
import type { BrowserClientMessage } from "./browser-protocol";
import type { BrowserFrameHandler } from "./use-browser-session";

type BrowserViewportProps = {
  /** 页面视口宽度（CSS 像素） */
  pageWidth: number;
  /** 页面视口高度（CSS 像素） */
  pageHeight: number;
  disabled: boolean;
  onSend: (message: BrowserClientMessage) => void;
  onFrameHandler: (handler: BrowserFrameHandler | null) => void;
};

/** 视口尺寸变化后发送 resize 的防抖时长。 */
const RESIZE_DEBOUNCE_MS = 200;

/**
 * 浏览器画面视口：绘制服务端推送的 JPEG 帧，并把鼠标、滚轮与键盘输入转发给页面。
 *
 * 键盘输入经过一个视觉隐藏的 textarea，借此获得输入法组合与粘贴事件；
 * 画布本身只负责显示与指针事件。
 *
 * @param props 页面尺寸、是否禁用、发送方法与画面回调注册方法
 * @returns 视口
 */
export function BrowserViewport({ pageWidth, pageHeight, disabled, onSend, onFrameHandler }: BrowserViewportProps) {
  const { t } = useI18n();
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const [hasFrame, setHasFrame] = useState(false);
  const [focused, setFocused] = useState(false);
  const sendRef = useRef(onSend);
  sendRef.current = onSend;
  const pageRef = useRef({ width: pageWidth, height: pageHeight });
  pageRef.current = { width: pageWidth, height: pageHeight };

  // 1. 注册画面回调：解码中到达的帧只保留最新一张，避免积压
  useEffect(() => {
    let decoding = false;
    let pending: Blob | null = null;
    let disposed = false;
    const draw = async (frame: Blob) => {
      decoding = true;
      try {
        const bitmap = await createImageBitmap(frame);
        const canvas = canvasRef.current;
        if (!disposed && canvas) {
          if (canvas.width !== bitmap.width) canvas.width = bitmap.width;
          if (canvas.height !== bitmap.height) canvas.height = bitmap.height;
          canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
          setHasFrame(true);
        }
        bitmap.close();
      } catch {
        // 单帧损坏时跳过，下一帧会覆盖
      } finally {
        decoding = false;
        const next = pending;
        pending = null;
        if (next && !disposed) void draw(next);
      }
    };
    onFrameHandler((frame) => {
      if (decoding) pending = frame;
      else void draw(frame);
    });
    return () => {
      disposed = true;
      onFrameHandler(null);
    };
  }, [onFrameHandler]);

  // 2. 视口尺寸跟随面板大小，防抖后通知服务端调整页面视口
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let timer: number | undefined;
    let last = { width: 0, height: 0 };
    const observer = new ResizeObserver(([entry]) => {
      const width = Math.floor(entry.contentRect.width);
      const height = Math.floor(entry.contentRect.height);
      if (width < 50 || height < 50) return;
      if (Math.abs(width - last.width) < 2 && Math.abs(height - last.height) < 2) return;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        last = { width, height };
        sendRef.current({ type: "resize", width, height });
      }, RESIZE_DEBOUNCE_MS);
    });
    observer.observe(host);
    return () => {
      window.clearTimeout(timer);
      observer.disconnect();
    };
  }, []);

  // 3. 指针与滚轮转发；滚轮需要非 passive 监听才能阻止面板自身滚动
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || disabled) return;
    const geometry = (): ViewportGeometry => {
      const rect = canvas.getBoundingClientRect();
      return {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        pageWidth: pageRef.current.width,
        pageHeight: pageRef.current.height
      };
    };
    let moveFrame = 0;
    let lastMove: MouseEvent | null = null;
    const onDown = (event: MouseEvent) => {
      event.preventDefault();
      inputRef.current?.focus({ preventScroll: true });
      sendRef.current(mouseMessage("down", event, geometry()));
    };
    const onUp = (event: MouseEvent) => sendRef.current(mouseMessage("up", event, geometry()));
    // 鼠标移动按动画帧合并，避免高频事件占满连接
    const onMove = (event: MouseEvent) => {
      lastMove = event;
      if (moveFrame) return;
      moveFrame = requestAnimationFrame(() => {
        moveFrame = 0;
        if (lastMove) sendRef.current(mouseMessage("move", lastMove, geometry()));
      });
    };
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      sendRef.current(mouseMessage("wheel", event, geometry()));
    };
    const onContextMenu = (event: MouseEvent) => event.preventDefault();
    canvas.addEventListener("mousedown", onDown);
    canvas.addEventListener("mouseup", onUp);
    canvas.addEventListener("mousemove", onMove);
    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("contextmenu", onContextMenu);
    return () => {
      cancelAnimationFrame(moveFrame);
      canvas.removeEventListener("mousedown", onDown);
      canvas.removeEventListener("mouseup", onUp);
      canvas.removeEventListener("mousemove", onMove);
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("contextmenu", onContextMenu);
    };
  }, [disabled]);

  /** 键盘事件转发；粘贴快捷键放行，由 paste 事件读取剪贴板文本。 */
  const forwardKey = (kind: "down" | "up", event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (disabled) return;
    const native = event.nativeEvent;
    if (kind === "down" && isNativePasteShortcut(native)) return;
    const message = keyMessage(kind, native);
    if (!message) return;
    event.preventDefault();
    onSend(message);
  };

  return (
    <div ref={hostRef} className={`browser-viewport${focused ? " focused" : ""}`}>
      <canvas
        ref={canvasRef}
        className="browser-canvas"
        style={{ aspectRatio: `${pageWidth} / ${pageHeight}` }}
        aria-label={t("Browser page", "浏览器页面")}
        role="img"
      />
      {!hasFrame && <p className="browser-viewport-placeholder">{t("Waiting for the page…", "等待页面画面…")}</p>}
      <textarea
        ref={inputRef}
        className="browser-key-sink"
        aria-label={t("Type into the browser page", "向浏览器页面输入")}
        tabIndex={disabled ? -1 : 0}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        onKeyDown={(event) => forwardKey("down", event)}
        onKeyUp={(event) => forwardKey("up", event)}
        onPaste={(event) => {
          event.preventDefault();
          const text = event.clipboardData.getData("text/plain");
          if (text && !disabled) onSend({ type: "insert_text", text });
        }}
        onCompositionEnd={(event) => {
          if (event.data && !disabled) onSend({ type: "insert_text", text: event.data });
          event.currentTarget.value = "";
        }}
        onInput={(event) => {
          // 组合输入之外的残留字符已经通过按键事件转发，清空即可
          if (!(event.nativeEvent as InputEvent).isComposing) event.currentTarget.value = "";
        }}
      />
    </div>
  );
}
