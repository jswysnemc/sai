const { BrowserWindow, WebContentsView, Menu, screen } = require('electron');
const path = require('node:path');
const { configureWorkbench } = require('./workbench-policy.cjs');
const { installControls } = require('./window-controls.cjs');

const TITLEBAR_HEIGHT = 40;

/**
 * 【桌面端】【自定义窗口】使用独立标题栏与 WebContentsView，避免修改 Sai 页面布局
 * @param {URL} url Sai 后端启动地址
 * @param {object} logger 日志接口
 * @returns {Promise<BrowserWindow>} 已加载的桌面窗口
 */
async function createWindow(url, logger) {
  const { width, height } = screen.getPrimaryDisplay().workAreaSize;
  const window = new BrowserWindow({
    width: Math.min(1280, width), height: Math.min(860, height),
    minWidth: Math.min(520, width), minHeight: Math.min(420, height),
    frame: false, show: false, title: 'Sai Desktop', backgroundColor: '#f7f7f7',
    icon: path.join(__dirname, '../../build/icon.png'),
    webPreferences: { preload: path.join(__dirname, '../preload/shell.cjs'),
      nodeIntegration: false, contextIsolation: true, sandbox: true, webviewTag: false },
  });
  const view = new WebContentsView({ webPreferences: {
    partition: 'persist:sai-desktop', preload: path.join(__dirname, '../preload/workbench.cjs'),
    nodeIntegration: false, contextIsolation: true, sandbox: true, webSecurity: true, webviewTag: false,
  } });
  window.contentView.addChildView(view);
  /**
   * 【桌面端】【视口布局】为标题栏保留高度，业务页面继续使用自己的完整视口
   * @returns {void} 无返回值
   */
  function layout() {
    const [contentWidth, contentHeight] = window.getContentSize();
    view.setBounds({ x: 0, y: TITLEBAR_HEIGHT, width: contentWidth, height: Math.max(1, contentHeight - TITLEBAR_HEIGHT) });
  }
  layout();
  window.on('resize', layout);
  window.setMenu(null);
  Menu.setApplicationMenu(process.platform === 'darwin'
    ? Menu.buildFromTemplate([{ role: 'appMenu' }, { role: 'editMenu' }]) : null);
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.session.setPermissionCheckHandler(() => false);
  window.webContents.session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
  window.webContents.session.setDevicePermissionHandler(() => false);
  window.webContents.on('will-navigate', (event) => event.preventDefault());
  await configureWorkbench(view.webContents, url, logger);
  const cleanup = installControls(window, view.webContents, url.origin);
  window.once('closed', () => { cleanup(); if (!view.webContents.isDestroyed()) view.webContents.close(); });
  await window.loadFile(path.join(__dirname, '../../renderer/dist/index.html'));
  window.show();
  await view.webContents.loadURL(url.href);
  view.webContents.focus();
  return window;
}

module.exports = { createWindow, TITLEBAR_HEIGHT };
