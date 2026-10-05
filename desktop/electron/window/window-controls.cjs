const { ipcMain, shell } = require('electron');
const { isInternal } = require('../navigation.cjs');

const ACTIONS = new Set(['back', 'forward', 'reload', 'minimize', 'maximize', 'close', 'help', 'browser', 'terminal']);
const COLORS = new Set(['surface', 'paper', 'ink', 'muted', 'line']);

/**
 * 【桌面适配】【颜色校验】只接受固定主题键与合法颜色表达式
 * @param {object} appearance 工作台上报的主题
 * @returns {object} 可供标题栏使用的颜色
 */
function sanitizeAppearance(appearance) {
  return Object.fromEntries(Object.entries(appearance || {}).filter(([name, value]) =>
    COLORS.has(name) && typeof value === 'string' && value.length < 160
    && /^(#[\da-f]{3,8}|(?:rgb|rgba|hsl|hsla|color|color-mix|oklch)\([\w\s.,%#()-]+\))$/i.test(value)));
}

/**
 * 【桌面适配】【窗口控制】注册仅标题栏主框架可调用的窗口接口及工作台主题接口
 * @param {BrowserWindow} window 桌面主窗口
 * @param {WebContents} workbench Sai 页面
 * @param {string} origin Sai 后端源地址
 * @returns {Function} 主窗口销毁时调用的清理函数
 */
function installControls(window, workbench, origin) {
  let appearance = {};
  const ownedShell = (event) => event.sender === window.webContents && event.senderFrame === window.webContents.mainFrame;
  const ownedWorkbench = (event) => event.sender === workbench && event.senderFrame === workbench.mainFrame
    && isInternal(event.senderFrame.url, origin);
  const state = () => ({ maximized: window.isMaximized(), canGoBack: workbench.navigationHistory.canGoBack(),
    canGoForward: workbench.navigationHistory.canGoForward(), loading: workbench.isLoading(),
    title: (workbench.getTitle() || 'Sai').replace(/^Sai Web$/, 'Sai'), appearance });
  const publish = () => { if (!window.isDestroyed() && !workbench.isDestroyed()) window.webContents.send('desktop:state-changed', state()); };

  /**
   * 【桌面适配】【动作执行】按白名单调用窗口或工作台能力
   * @param {string} action 动作名称
   * @returns {Promise<void>} 操作结束后完成
   */
  async function perform(action) {
    if (!ACTIONS.has(action)) throw new Error('不支持的桌面操作');
    switch (action) {
      case 'back': if (workbench.navigationHistory.canGoBack()) workbench.navigationHistory.goBack(); break;
      case 'forward': if (workbench.navigationHistory.canGoForward()) workbench.navigationHistory.goForward(); break;
      case 'reload': workbench.reload(); break;
      case 'minimize': window.minimize(); break;
      case 'maximize': window.isMaximized() ? window.unmaximize() : window.maximize(); break;
      case 'close': window.close(); break;
      case 'help': await shell.openExternal('https://github.com/jswysnemc/sai/blob/main/desktop/README.md'); break;
      case 'browser': case 'terminal': workbench.send('desktop:open-panel', action); workbench.focus(); break;
    }
  }
  ipcMain.handle('desktop:state', (event) => {
    if (!ownedShell(event)) throw new Error('窗口接口只允许标题栏调用');
    return state();
  });
  ipcMain.handle('desktop:action', (event, action) => {
    if (!ownedShell(event)) throw new Error('窗口接口只允许标题栏调用');
    return perform(action);
  });
  const receiveAppearance = (event, colors) => {
    if (!ownedWorkbench(event)) return;
    appearance = sanitizeAppearance(colors);
    if (/^#[\da-f]{6}$/i.test(appearance.paper || '')) window.setBackgroundColor(appearance.paper);
    publish();
  };
  ipcMain.on('desktop:appearance', receiveAppearance);
  for (const event of ['maximize', 'unmaximize', 'restore', 'enter-full-screen', 'leave-full-screen']) window.on(event, publish);
  for (const event of ['did-finish-load', 'did-navigate', 'did-navigate-in-page', 'did-start-loading', 'did-stop-loading', 'page-title-updated']) workbench.on(event, publish);
  window.webContents.on('did-finish-load', publish);

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
    }
  }
  window.webContents.on('before-input-event', shortcut);
  workbench.on('before-input-event', shortcut);
  return () => {
    ipcMain.removeHandler('desktop:state');
    ipcMain.removeHandler('desktop:action');
    ipcMain.removeListener('desktop:appearance', receiveAppearance);
  };
}

module.exports = { installControls, sanitizeAppearance };
