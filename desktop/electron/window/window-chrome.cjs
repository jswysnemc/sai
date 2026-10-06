/** 三个窗口按钮各 46px，与 Windows 标题栏按钮同宽 */
const CONTROLS_WIDTH = 138;
/** macOS 红绿灯占用的左侧宽度 */
const TRAFFIC_LIGHT_WIDTH = 78;
/** 工作台未上报时使用的顶行高度（2rem） */
const DEFAULT_ROW_HEIGHT = 32;

/**
 * 【桌面端】【顶行布局】工作台铺满窗口，按钮视图贴右上角并与工作台顶行等高
 * @param {BrowserWindow} window 桌面主窗口
 * @param {WebContentsView} view 工作台视图
 * @param {WebContentsView|null} controls 窗口按钮视图；macOS 为 null
 * @returns {object} 布局与顶行度量接口
 */
function createChromeLayout(window, view, controls) {
  // 顶行高度以工作台 CSS 像素为准，乘缩放系数得到窗口坐标
  let rowCss = DEFAULT_ROW_HEIGHT;

  /**
   * 【桌面端】【顶行布局】按窗口尺寸与缩放重排两个视图
   * @returns {void} 无返回值
   */
  function layout() {
    const [width, height] = window.getContentSize();
    view.setBounds({ x: 0, y: 0, width, height });
    if (!controls) return;
    const zoom = view.webContents.isDestroyed() ? 1 : view.webContents.getZoomFactor();
    const rowHeight = Math.max(24, Math.round(rowCss * zoom));
    controls.setBounds({ x: Math.max(0, width - CONTROLS_WIDTH), y: 0, width: Math.min(width, CONTROLS_WIDTH), height: rowHeight });
  }

  /**
   * 【桌面端】【顶行让位】返回工作台需要在顶行两端让出的 CSS 宽度
   * @returns {object} 平台与左右让位宽度
   */
  function insets() {
    const zoom = view.webContents.isDestroyed() ? 1 : view.webContents.getZoomFactor() || 1;
    const right = controls ? CONTROLS_WIDTH : 0;
    const left = controls ? 0 : TRAFFIC_LIGHT_WIDTH;
    return { platform: process.platform, right: right / zoom, left: left / zoom };
  }

  /**
   * 【桌面端】【顶行高度】接收工作台上报的顶行 CSS 高度
   * @param {number} value 顶行高度
   * @returns {void} 无返回值
   */
  function setRowHeight(value) {
    if (!Number.isFinite(value) || value < 16 || value > 96) return;
    rowCss = value;
    layout();
  }

  window.on('resize', layout);
  layout();
  return { layout, insets, setRowHeight };
}

module.exports = { createChromeLayout, CONTROLS_WIDTH, TRAFFIC_LIGHT_WIDTH };
