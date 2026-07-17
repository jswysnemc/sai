import { X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import "./image-lightbox.css";

type ImageLightboxProps = {
  src: string;
  alt?: string;
  onClose: () => void;
};

type ViewState = {
  scale: number;
  x: number;
  y: number;
};

const MIN_SCALE = 0.2;
const MAX_SCALE = 6;

/**
 * 全屏图片查看遮罩，支持滚轮缩放、拖动平移和多种关闭方式。
 *
 * @param props src 为图片地址，alt 为替代文本，onClose 为关闭回调
 * @returns 全屏图片查看层
 */
export function ImageLightbox({ src, alt, onClose }: ImageLightboxProps) {
  const [view, setView] = useState<ViewState>({ scale: 1, x: 0, y: 0 });
  const dragRef = useRef<{ startX: number; startY: number; originX: number; originY: number } | null>(null);
  const [dragging, setDragging] = useState(false);

  // 1. 监听 Esc 键关闭
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  // 2. 滚轮缩放，限制在 0.2 到 6 倍之间
  const onWheel = useCallback((event: React.WheelEvent) => {
    setView((current) => {
      const factor = event.deltaY < 0 ? 1.12 : 1 / 1.12;
      const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, current.scale * factor));
      return { ...current, scale };
    });
  }, []);

  // 3. 指针拖动平移图片
  const onPointerDown = useCallback((event: React.PointerEvent) => {
    event.preventDefault();
    (event.target as Element).setPointerCapture(event.pointerId);
    setView((current) => {
      dragRef.current = { startX: event.clientX, startY: event.clientY, originX: current.x, originY: current.y };
      return current;
    });
    setDragging(true);
  }, []);

  const onPointerMove = useCallback((event: React.PointerEvent) => {
    const drag = dragRef.current;
    if (!drag) return;
    setView((current) => ({ ...current, x: drag.originX + event.clientX - drag.startX, y: drag.originY + event.clientY - drag.startY }));
  }, []);

  const onPointerUp = useCallback(() => {
    dragRef.current = null;
    setDragging(false);
  }, []);

  return (
    <div className="image-lightbox" role="dialog" aria-label={alt || "图片查看"} onClick={onClose} onWheel={onWheel}>
      <button type="button" className="image-lightbox-close" aria-label="关闭图片" onClick={onClose}>
        <X size={18} />
      </button>
      <img
        className={`image-lightbox-image${dragging ? " dragging" : ""}`}
        src={src}
        alt={alt ?? ""}
        style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})` }}
        draggable={false}
        onClick={(event) => event.stopPropagation()}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      />
    </div>
  );
}

type LightboxImageProps = {
  src: string;
  alt?: string;
  className?: string;
};

/**
 * 可点击放大的图片元素，点击后打开全屏查看层。
 *
 * @param props src 为图片地址，alt 为替代文本，className 为缩略图样式类
 * @returns 缩略图与按需渲染的查看层
 */
export function LightboxImage({ src, alt, className }: LightboxImageProps) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <img className={`${className ?? ""} lightbox-trigger`.trim()} src={src} alt={alt ?? ""} onClick={() => setOpen(true)} />
      {open && <ImageLightbox src={src} alt={alt} onClose={() => setOpen(false)} />}
    </>
  );
}
