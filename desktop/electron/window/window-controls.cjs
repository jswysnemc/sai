const { ipcMain } = require('electron');
const { isInternal } = require('../navigation.cjs');

const ACTIONS = new Set(['minimize', 'maximize', 'close']);
const COLORS = new Set(['surface', 'paper', 'ink', 'muted', 'line']);

/**
 * 【桌面适配】【颜色校验】只接受固定主题键与合法颜色表达式
 * @param {object} appearance 工作台上报的主题
 * @returns {object} 可供窗口按钮使用的颜色
 */
function sanitizeAppearance(appearance) {
  return Object.fromEntries(Object.entries(appearance || {}).filter(([name, value]) =>
    COLORS.has(name) && typeof value === 'string' && value.length < 160
    && /^(#[\da-f]{3,8}|(?:rgb|rgba|hsl|hsla|color|color-mix|oklch)\([\w\s.,%#()-]+\))$/i.test(value)));
}

/**
 * 【桌面适配】【窗口控制】注册窗口按钮接口、工作台主题与顶行度量同步
 * @param {BrowserWindow} window 桌面主窗口
 * @param {WebContents} workbench Sai 页面
 * @param {WebContents|null} controls 窗口按钮页面；macOS 为 null
 * @param {string} origin Sai 后端源地址
 * @param {object} chrome 顶行布局接口
 * @returns {Function} 主窗口销毁时调用的清理函数
 */
function installControls(window, workbench, controls, origin, chrome) {
  let appearance = {};
  const ownedControls = (event) => Boolean(controls) && event.sender === controls
    && event.senderFrame === controls.mainFrame;
  const ownedWorkbench = (event) => event.sender === workbench && event.senderFrame === workbench.mainFrame
    && isInternal(event.senderFrame.url, origin);
  const state = () => ({ maximized: window.isMaximized(), appearance });
  const publish = () => {
    if (controls && !window.isDestroyed() && !controls.isDestroyed()) controls.send('desktop:state-changed', state());
  };
  // 1. 工作台需要知道顶行两端为窗口按钮让出多少宽度
  const syncInsets = () => {
    if (window.isDestroyed() || workbench.isDestroyed()) return;
    chrome.layout();
    workbench.send('desktop:chrome', chrome.insets());
  };

  /**
   * 【桌面适配】【动作执行】按白名单调用窗口能力
   * @param {string} action 动作名称
   * @returns {void} 无返回值
   */
  function perform(action) {
    if (!ACTIONS.has(action)) throw new Error('不支持的桌面操作');
    if (action === 'minimize') window.minimize();
    else if (action === 'maximize') window.isMaximized() ? window.unmaximize() : window.maximize();
    else window.close();
  }
  ipcMain.handle('desktop:state', (event) => {
    if (!ownedControls(event)) throw new Error('窗口接口只允许窗口按钮调用');
    return state();
  });
  ipcMain.handle('desktop:action', (event, action) => {
    if (!ownedControls(event)) throw new Error('窗口接口只允许窗口按钮调用');
    return perform(action);
  });
  const receiveAppearance = (event, colors) => {
    if (!ownedWorkbench(event)) return;
    appearance = sanitizeAppearance(colors);
    if (/^#[\da-f]{6}$/i.test(appearance.paper || '')) window.setBackgroundColor(appearance.paper);
    publish();
  };
  const receiveChrome = (event, metrics) => {
    if (!ownedWorkbench(event)) return;
    chrome.setRowHeight(Number(metrics?.rowHeight));
    syncInsets();
  };
  // 2. 工作台标题行双击：Linux 无边框窗口不会自动最大化
  const receiveToggle = (event) => {
    if (ownedWorkbench(event)) perform('maximize');
  };
  ipcMain.on('desktop:appearance', receiveAppearance);
  ipcMain.on('desktop:chrome-metrics', receiveChrome);
  ipcMain.on('desktop:toggle-maximize', receiveToggle);
  for (const event of ['maximize', 'unmaximize', 'restore', 'enter-full-screen', 'leave-full-screen']) window.on(event, publish);
  workbench.on('did-finish-load', syncInsets);
  workbench.on('zoom-changed', syncInsets);
  controls?.on('did-finish-load', publish);

  /**
   * 【桌面适配】【快捷键】移除菜单栏后保留刷新、缩放和开发工具快捷键
   * @param {Event} event Electron 输入事件
   * @param {object} input 按键信息
   * @returns {void} 无返回值
   */
  function shortcut(event, input) {
    if (input.type !== 'keyDown') return;
    const modifier = process.platform === 'darwin' ? input.meta : input.control;
    const key = input.key.toLowerCase();
    if (modifier && key === 'r') { event.preventDefault(); workbench.reload(); }
    else if (modifier && input.shift && key === 'i') { event.preventDefault(); workbench.toggleDevTools(); }
    else if (key === 'f11') { event.preventDefault(); window.setFullScreen(!window.isFullScreen()); }
    else if (modifier && ['+', '=', '-', '0'].includes(key)) {
      event.preventDefault();
      workbench.setZoomLevel(key === '0' ? 0 : Math.max(-4, Math.min(5, workbench.getZoomLevel() + (key === '-' ? -0.5 : 0.5))));
      syncInsets();
    }
  }
  workbench.on('before-input-event', shortcut);
  controls?.on('before-input-event', shortcut);
  return () => {
    ipcMain.removeHandler('desktop:state');
    ipcMain.removeHandler('desktop:action');
    ipcMain.removeListener('desktop:appearance', receiveAppearance);
    ipcMain.removeListener('desktop:chrome-metrics', receiveChrome);
    ipcMain.removeListener('desktop:toggle-maximize', receiveToggle);
  };
}

module.exports = { installControls, sanitizeAppearance };
