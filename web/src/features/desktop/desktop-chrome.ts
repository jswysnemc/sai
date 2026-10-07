import { rowInset, type DesktopReserve } from "./desktop-chrome-insets";
import "./desktop-chrome.css";

// ===== 新增 import =====
import { needsCaptionHitHole } from "./desktop-chrome-insets";

/** 可能贴着窗口顶端的顶行；与桌面预加载脚本的拖动区保持一致。 */
const TOP_ROWS = ".chat-header, .workspace-tab-bar, .settings-topbar, .sidebar-heading";
/** 记录元素原始内边距的数据键，恢复时使用。 */
const ORIGINAL_PADDING = "desktopPaddingOriginal";

/**
 * 读取根节点上的长度变量（视口像素）。
 *
 * @param style 根节点计算样式
 * @param name 变量名
 * @returns 像素值；未设置时为 0
 */
function readPx(style: CSSStyleDeclaration, name: string): number {
  return Number.parseFloat(style.getPropertyValue(name)) || 0;
}

/**
 * 读取桌面外壳写入的窗口按钮区域。
 *
 * @param root 文档根节点
 * @returns 窗口按钮区域，单位为视口像素
 */
function readReserve(root: HTMLElement): DesktopReserve {
  const style = getComputedStyle(root);
  const probe = document.createElement("div");
  probe.style.cssText = "position:absolute;visibility:hidden;height:var(--toolbar-height, 2rem)";
  root.appendChild(probe);
  const height = probe.getBoundingClientRect().height;
  probe.remove();
  return {
    right: readPx(style, "--desktop-inset-right"),
    left: readPx(style, "--desktop-inset-left"),
    height
  };
}

/**
 * 按重叠宽度给顶行元素补内边距，没有重叠时恢复原值。
 *
 * 内边距写在元素自身上，背景色随之延伸到窗口按钮下方，看起来是同一条标题栏。
 * 根节点有 CSS zoom 时，视口像素需要换算回元素坐标。
 *
 * @param element 顶行元素
 * @param extra 视口像素下的左右额外宽度
 * @param zoom 根节点缩放
 */
function applyRowInset(element: HTMLElement, extra: { left: number; right: number }, zoom: number) {
  const dataset = element.dataset;
  if (!extra.left && !extra.right) {
    if (dataset[ORIGINAL_PADDING] === undefined) return;
    element.style.paddingLeft = "";
    element.style.paddingRight = "";
    delete dataset[ORIGINAL_PADDING];
    delete dataset.desktopEdge;
    return;
  }
  // 1. 首次让位时记下原始内边距，之后都在原值上叠加
  if (dataset[ORIGINAL_PADDING] === undefined) {
    const style = getComputedStyle(element);
    dataset[ORIGINAL_PADDING] = `${Number.parseFloat(style.paddingLeft) || 0},${Number.parseFloat(style.paddingRight) || 0}`;
  }
  const [left, right] = dataset[ORIGINAL_PADDING].split(",").map(Number);
  const nextLeft = extra.left ? `${left + extra.left / zoom}px` : "";
  const nextRight = extra.right ? `${right + extra.right / zoom}px` : "";
  if (element.style.paddingLeft !== nextLeft) element.style.paddingLeft = nextLeft;
  if (element.style.paddingRight !== nextRight) element.style.paddingRight = nextRight;
  dataset.desktopEdge = extra.left && extra.right ? "both" : extra.right ? "right" : "left";
}

/**
 * 重新计算所有顶行元素的让位宽度。
 *
 * @param root 文档根节点
 */
function refresh(root: HTMLElement) {
  const reserve = readReserve(root);
  const zoom = Number.parseFloat(getComputedStyle(root).zoom) || 1;
  for (const element of document.querySelectorAll<HTMLElement>(TOP_ROWS)) {
    // 2. 让位只改内边距，不影响元素外框，可直接按外框判断重叠
    const rect = element.getBoundingClientRect();
    applyRowInset(element, rowInset(rect, window.innerWidth, reserve), zoom);
  }
}

/**
 * 【桌面端】【一体化标题栏】在桌面外壳中让工作台顶行避开窗口按钮。
 *
 * 浏览器访问时没有 data-desktop 标记，不做任何事。布局变化（侧栏收起、
 * 面板交换、窗口缩放、页面切换）都会重新计算哪个顶行贴着窗口按钮。
 *
 * @returns 停止监听的清理函数
 */
export function enableDesktopChrome(): () => void {
  const root = document.documentElement;
  if (!root.dataset.desktop) {
    // 1. 桌面外壳的标记可能晚于本脚本写入，到达后再启用
    const waiter = new MutationObserver(() => {
      if (!root.dataset.desktop) return;
      waiter.disconnect();
      stop = start(root);
    });
    let stop: () => void = () => waiter.disconnect();
    waiter.observe(root, { attributes: true, attributeFilter: ["data-desktop"] });
    return () => stop();
  }
  return start(root);
}

/**
 * 开始监听布局变化并计算顶行让位。
 *
 * @param root 文档根节点
 * @returns 停止监听的清理函数
 */
function start(root: HTMLElement): () => void {
  const removeShield = mountCaptionShield(root);
  let timer = 0;
  // 1. 用定时器合并重算：嵌入视图在后台或未合成时 requestAnimationFrame 可能不触发，
  //    首帧卡住会让之后的重算全部被跳过，窗口按钮因此压在顶行控件上
  const schedule = () => {
    if (timer) return;
    timer = window.setTimeout(() => {
      timer = 0;
      refresh(root);
    }, 16);
  };
  // 2. 只监听结构与类名变化，不监听 style，避免自身写入内边距引起循环
  const observer = new MutationObserver(schedule);
  observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["class", "hidden"] });
  const rootObserver = new MutationObserver(schedule);
  rootObserver.observe(root, { attributes: true, attributeFilter: ["style", "class", "data-theme"] });
  const resize = new ResizeObserver(schedule);
  resize.observe(document.body);
  // 3. 窗口尺寸变化可能跨过响应式断点，原始内边距需要重新读取
  const onResize = () => {
    for (const element of document.querySelectorAll<HTMLElement>(TOP_ROWS)) applyRowInset(element, { left: 0, right: 0 }, 1);
    schedule();
  };
  window.addEventListener("resize", onResize);
  document.addEventListener("transitionend", schedule, true);
  schedule();
  return () => {
    if (timer) window.clearTimeout(timer);
    observer.disconnect();
    removeShield();
    rootObserver.disconnect();
    resize.disconnect();
    window.removeEventListener("resize", onResize);
    document.removeEventListener("transitionend", schedule, true);
  };
}

/**
 * 在 Windows 右上角放一块不可拖动的区域，避免标题栏按钮被当成窗口拖动。
 *
 * @param root 文档根节点
 * @returns 移除该区域的函数
 */
function mountCaptionShield(root: HTMLElement): () => void {
  if (!needsCaptionHitHole(root.dataset.desktop ?? "") || !document.body) return () => {};
  const shield = document.createElement("div");
  shield.className = "desktop-caption-shield";
  shield.setAttribute("aria-hidden", "true");
  document.body.appendChild(shield);
  return () => shield.remove();
}
