const { ipcRenderer } = require('electron');

/** 工作台顶行里可拖动窗口的区域；与 Web 端 desktop-chrome.css 保持一致 */
const DRAG_ROWS = '.chat-header, .workspace-tab-bar, .settings-topbar, .sidebar-heading';
/** 上次上报的顶行高度；未变化时不重复上报 */
let reportedRowHeight = 0;
/** 顶行内仍需正常点击的控件 */
const INTERACTIVE = 'button, a, input, select, textarea, [role="tab"], [role="button"], [contenteditable="true"]';

/**
 * 【桌面适配】【主题同步】只上报明确的配色变量，不向业务页面暴露窗口操作接口
 * @returns {void} 无返回值
 */
function reportAppearance() {
  const style = getComputedStyle(document.documentElement);
  const read = (name) => style.getPropertyValue(name).trim();
  ipcRenderer.send('desktop:appearance', {
    surface: read('--sidebar-surface'), paper: read('--paper'), ink: read('--ink'),
    muted: read('--ink-soft'), line: read('--line'),
  });
}

/**
 * 【桌面适配】【顶行度量】上报工作台顶行高度，窗口按钮视图与之等高
 * @returns {void} 无返回值
 */
function reportChromeMetrics() {
  const probe = document.createElement('div');
  probe.style.cssText = 'position:absolute;visibility:hidden;height:var(--toolbar-height, 2rem)';
  document.documentElement.appendChild(probe);
  const rowHeight = probe.getBoundingClientRect().height;
  probe.remove();
  if (!rowHeight || rowHeight === reportedRowHeight) return;
  reportedRowHeight = rowHeight;
  ipcRenderer.send('desktop:chrome-metrics', { rowHeight });
}

/**
 * 【桌面适配】【顶行让位】写入顶行两端需要让出的宽度，样式由 Web 端负责
 * @param {object} insets 平台与左右宽度
 * @returns {void} 无返回值
 */
function applyInsets(insets) {
  const root = document.documentElement;
  const px = (value) => `${Math.max(0, Number(value) || 0).toFixed(2)}px`;
  // 写入根节点 style 会触发主题监听，值未变化时跳过以免循环
  for (const [name, value] of [['--desktop-inset-right', px(insets?.right)], ['--desktop-inset-left', px(insets?.left)]]) {
    if (root.style.getPropertyValue(name) !== value) root.style.setProperty(name, value);
  }
}

if (process.isMainFrame) {
  // 1. 预加载时根节点可能尚未由解析器创建；解析完成（早于页面模块脚本）时再补一次标记
  const markDesktop = () => {
    if (document.documentElement) document.documentElement.dataset.desktop = process.platform;
  };
  markDesktop();
  document.addEventListener('readystatechange', markDesktop);
  ipcRenderer.on('desktop:chrome', (_event, insets) => applyInsets(insets));
  window.addEventListener('DOMContentLoaded', () => {
    reportAppearance();
    reportChromeMetrics();
    const observer = new MutationObserver(() => { reportAppearance(); reportChromeMetrics(); });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', reportAppearance);
    window.addEventListener('load', reportAppearance, { once: true });
  });
  // 2. Linux 无边框窗口不会响应拖动区双击，这里补上最大化切换
  if (process.platform === 'linux') {
    window.addEventListener('dblclick', (event) => {
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest(DRAG_ROWS) && !target.closest(INTERACTIVE)) ipcRenderer.send('desktop:toggle-maximize');
    });
  }
}
