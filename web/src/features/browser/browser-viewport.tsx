import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { useI18n } from "../i18n/use-i18n";
import { isNativePasteShortcut, keyMessage, mouseMessage, type ViewportGeometry } from "./browser-input";
import type { BrowserClientMessage } from "./browser-protocol";
import type { BrowserFrameHandler } from "./use-browser-session";
import { useBrowserResize } from "./use-browser-resize";
import { displayScale, type ResponsiveViewport } from "./browser-responsive";

type BrowserViewportProps = {
  /** 页面视口宽度（CSS 像素） */
  pageWidth: number;
  /** 页面视口高度（CSS 像素） */
  pageHeight: number;
  disabled: boolean;
  onSend: (message: BrowserClientMessage) => void;
  onFrameHandler: (handler: BrowserFrameHandler | null) => void;
  /** 自由尺寸模式；为空时页面视口跟随面板大小 */
  responsive: ResponsiveViewport | null;
  /** 浮在画面上方的内容（下拉菜单等），坐标按画面缩放后的位置换算 */
  overlay?: (scale: number) => ReactNode;
};

/**
 * 浏览器画面视口：绘制服务端推送的 JPEG 帧，并把鼠标、滚轮与键盘输入转发给页面。
 *
 * 键盘输入经过一个视觉隐藏的 textarea，借此获得输入法组合与粘贴事件；
 * 画布本身只负责显示与指针事件。
 *
 * @param props 页面尺寸、是否禁用、发送方法与画面回调注册方法
 * @returns 视口
 */
export function BrowserViewport({ pageWidth, pageHeight, disabled, onSend, onFrameHandler, responsive, overlay }: BrowserViewportProps) {
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

  const hostSize = useBrowserResize(hostRef, !disabled, onSend, responsive?.size ?? null);
  // 自由尺寸时按缩放档位显示；跟随面板时画面与面板一比一
  const scale = responsive ? displayScale({ width: pageWidth, height: pageHeight }, hostSize, responsive.zoom) : 1;

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

  // 2. 指针与滚轮转发；滚轮需要非 passive 监听才能阻止面板自身滚动
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
    /**
     * 【浏览器面板】【移动提交】在点击边界前发送最后一次移动，避免延迟回调打乱顺序。
     * @returns 无
     */
    const flushMove = () => {
      cancelAnimationFrame(moveFrame);
      moveFrame = 0;
      if (lastMove) sendRef.current(mouseMessage("move", lastMove, geometry()));
      lastMove = null;
    };
    const onDown = (event: MouseEvent) => {
      flushMove();
      event.preventDefault();
      inputRef.current?.focus({ preventScroll: true });
      sendRef.current(mouseMessage("down", event, geometry()));
    };
    const onUp = (event: MouseEvent) => {
      flushMove();
      sendRef.current(mouseMessage("up", event, geometry()));
    };
    // 鼠标移动按动画帧合并，避免高频事件占满连接
    const onMove = (event: MouseEvent) => {
      lastMove = event;
      if (moveFrame) return;
      moveFrame = requestAnimationFrame(flushMove);
    };
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      flushMove();
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
    <div ref={hostRef} className={`browser-viewport${focused ? " focused" : ""}${responsive ? " responsive" : ""}`}>
      <div className="browser-canvas-frame" style={{ width: pageWidth * scale, height: pageHeight * scale }}>
        <canvas
          ref={canvasRef}
          className="browser-canvas"
          style={{ width: pageWidth * scale, height: pageHeight * scale }}
          aria-label={t("Browser page", "浏览器页面")}
          role="img"
        />
        {overlay?.(scale)}
      </div>
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
