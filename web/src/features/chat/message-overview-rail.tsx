import { type CSSProperties, type RefObject, useCallback, useLayoutEffect, useRef, useState } from "react";
import { evenlySpacedOverviewPosition, type MessageOverviewItem } from "./message-overview-utils";
import "./message-overview-rail.css";
import { useI18n } from "../i18n/use-i18n";

interface MessageOverviewRailProps {
  scrollContainerRef: RefObject<HTMLElement | null>;
  items: MessageOverviewItem[];
  activeId?: string;
  onNavigate?: () => void;
}

interface PositionedOverviewItem {
  item: MessageOverviewItem;
  top: number;
  active: boolean;
}

const TRACK_INSET = 6;

/**
 * 转义属性选择器中的字符串值，避免消息标识破坏选择器。
 *
 * @param value 需要转义的属性值
 * @returns 可安全写入双引号属性选择器的值
 */
function escapeAttributeValue(value: string) {
  return value.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
}

/**
 * 在滚动容器中查找概览项对应的消息锚点。
 *
 * @param container 消息滚动容器
 * @param itemId 概览项标识
 * @returns 匹配的消息元素，未找到时返回 null
 */
function findOverviewTarget(container: HTMLElement, itemId: string) {
  const selector = `[data-overview-id="${escapeAttributeValue(itemId)}"]`;
  return container.querySelector<HTMLElement>(selector);
}

/**
 * 判断两次轨道布局是否一致，减少滚动期间无意义的渲染。
 *
 * @param previous 上一次布局
 * @param next 新布局
 * @returns 两次布局是否一致
 */
function layoutsEqual(previous: PositionedOverviewItem[], next: PositionedOverviewItem[]) {
  if (previous.length !== next.length) return false;
  return previous.every((entry, index) => {
    const candidate = next[index];
    return candidate !== undefined
      && entry.item === candidate.item
      && entry.top === candidate.top
      && entry.active === candidate.active;
  });
}

/**
 * 渲染聊天消息概览轨道，并提供消息预览和快速跳转。
 *
 * @param props.scrollContainerRef 消息滚动容器引用
 * @param props.items 需要展示的消息概览项
 * @param props.activeId 外部指定的当前消息标识
 * @returns 消息概览轨道
 */
export function MessageOverviewRail({ scrollContainerRef, items, activeId, onNavigate }: MessageOverviewRailProps) {
  const { t } = useI18n();
  const railRef = useRef<HTMLElement>(null);
  const frameRef = useRef<number | null>(null);
  const itemsRef = useRef(items);
  const [positionedItems, setPositionedItems] = useState<PositionedOverviewItem[]>([]);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [visible, setVisible] = useState(false);
  const itemIds = items.map((item) => item.id).join("\u0000");
  itemsRef.current = items;

  /** 根据消息位置、滚动位置和轨道高度重新计算全部标记。 */
  const updatePositions = useCallback(() => {
    const container = scrollContainerRef.current;
    const rail = railRef.current;
    if (!container || !rail) {
      setPositionedItems([]);
      return;
    }

    // 1. 收集仍存在于当前会话 DOM 中的消息锚点
    const containerRect = container.getBoundingClientRect();
    const resolved = itemsRef.current.flatMap((item) => {
      const element = findOverviewTarget(container, item.id);
      if (!element) return [];
      const contentTop = element.getBoundingClientRect().top - containerRect.top + container.scrollTop;
      return [{ item, element, contentTop }];
    });
    const canNavigate = resolved.length > 1 && container.scrollHeight > container.clientHeight + 24;
    setVisible(canNavigate);
    if (!canNavigate) {
      setPositionedItems([]);
      return;
    }

    // 2. 未由外部指定时，以视口上部区域最后经过的消息作为当前项
    const viewportAnchor = container.scrollTop + container.clientHeight * 0.28;
    const automaticActive = resolved.reduce<(typeof resolved)[number] | undefined>((current, entry) => {
      if (entry.contentTop > viewportAnchor) return current;
      if (!current || entry.contentTop >= current.contentTop) return entry;
      return current;
    }, undefined) ?? resolved[0];
    const resolvedActiveId = activeId ?? automaticActive?.item.id;

    // 3. 按显示顺序将标识居中排列，并保持均匀的紧凑间距
    const trackHeight = Math.max(rail.clientHeight - TRACK_INSET * 2, 0);
    const next = resolved.map(({ item }, index) => ({
      item,
      top: TRACK_INSET + evenlySpacedOverviewPosition(index, resolved.length, trackHeight),
      active: item.id === resolvedActiveId
    }));
    setPositionedItems((previous) => layoutsEqual(previous, next) ? previous : next);
  }, [activeId, scrollContainerRef]);

  /** 将密集的滚动和尺寸变化合并到下一帧处理。 */
  const scheduleUpdate = useCallback(() => {
    if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = null;
      updatePositions();
    });
  }, [updatePositions]);

  useLayoutEffect(() => {
    const container = scrollContainerRef.current;
    const rail = railRef.current;
    if (!container || !rail) return;

    // 1. 滚动时更新当前项，尺寸变化时重新映射全部标记
    container.addEventListener("scroll", scheduleUpdate, { passive: true });
    const resizeObserver = new ResizeObserver(scheduleUpdate);
    resizeObserver.observe(container);
    resizeObserver.observe(rail);
    itemsRef.current.forEach((item) => {
      const element = findOverviewTarget(container, item.id);
      if (element) resizeObserver.observe(element);
    });
    scheduleUpdate();

    // 2. 会话切换或消息流式增长时重新绑定尚未出现的锚点
    const mutationObserver = new MutationObserver(scheduleUpdate);
    mutationObserver.observe(container, { childList: true, subtree: true });
    return () => {
      container.removeEventListener("scroll", scheduleUpdate);
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    };
  }, [itemIds, scheduleUpdate, scrollContainerRef]);

  // 3. 流式摘要变化时刷新预览数据，但不重复绑定全部观察器
  useLayoutEffect(() => scheduleUpdate(), [items, scheduleUpdate]);

  /**
   * 平滑滚动到指定消息。
   *
   * @param itemId 目标消息标识
   */
  const jumpToItem = (itemId: string) => {
    const container = scrollContainerRef.current;
    if (!container) return;
    const element = findOverviewTarget(container, itemId);
    if (!element) return;
    onNavigate?.();
    const containerRect = container.getBoundingClientRect();
    const top = element.getBoundingClientRect().top - containerRect.top + container.scrollTop - 12;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    container.scrollTo({ top, behavior: reducedMotion ? "auto" : "smooth" });
  };

  if (items.length === 0) return null;

  return (
    <nav ref={railRef} className={`message-overview-rail${visible ? "" : " is-hidden"}`} aria-label={t("Message overview", "消息概览")}>
      <ol className="message-overview-list">
        {positionedItems.map(({ item, top, active }) => {
          const style = { "--message-overview-top": `${top}px` } as CSSProperties;
          return (
            <li key={item.id} className="message-overview-entry" style={style} data-message-overview-id={item.id}>
              <button
                type="button"
                className={`message-overview-marker${active ? " is-active" : ""}`}
                onClick={() => jumpToItem(item.id)}
                onMouseEnter={() => setPreviewId(item.id)}
                onMouseLeave={() => setPreviewId((current) => current === item.id ? null : current)}
                onFocus={() => setPreviewId(item.id)}
                onBlur={() => setPreviewId((current) => current === item.id ? null : current)}
                aria-label={t(`Jump to ${item.title}`, `跳转到${item.title}`)}
                aria-current={active ? "location" : undefined}
                aria-describedby={previewId === item.id ? `message-overview-preview-${item.id}` : undefined}
              >
                <span className="message-overview-marker-line" aria-hidden="true" />
              </button>
              {previewId === item.id && <div id={`message-overview-preview-${item.id}`} className="message-overview-preview" role="tooltip">
                <div className="message-overview-preview-heading">
                  <span className="message-overview-preview-label">{item.label}</span>
                </div>
                <strong>{item.title}</strong>
                <p>{item.summary}</p>
                {item.tags.length > 0 && (
                  <div className="message-overview-preview-tags">
                    {item.tags.map((tag) => <span key={tag}>{tag}</span>)}
                    {item.hiddenTagCount > 0 && <span>+{item.hiddenTagCount}</span>}
                  </div>
                )}
              </div>}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
