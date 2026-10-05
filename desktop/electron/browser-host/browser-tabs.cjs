const { BrowserWindow } = require('electron');
const { EventEmitter } = require('node:events');

/**
 * 【桌面浏览器】【地址校验】限制页面顶层导航，保持与 Sai 地址策略一致
 * @param {string} value 页面地址
 * @returns {boolean} 是否允许作为页面打开
 */
function allowedPageUrl(value) {
  if (value === 'about:blank' || value.startsWith('about:blank#')) return true;
  try { return ['http:', 'https:'].includes(new URL(value).protocol); } catch { return false; }
}

/** 【桌面浏览器】【标签管理】只管理浏览页面，不包含桌面工作台与开发工具 */
class BrowserTabs extends EventEmitter {
  /**
   * 【桌面浏览器】【标签配置】记录独立会话与用户代理
   * @param {Session} session 浏览器专属 Electron 会话
   * @param {string} userAgent 页面使用的浏览器标识
   * @returns {BrowserTabs} 标签管理器
   */
  constructor(session, userAgent) {
    super();
    this.session = session;
    this.userAgent = userAgent;
    this.tabs = new Map();
  }

  /**
   * 【桌面浏览器】【窗口配置】创建沙盒离屏页面，关闭 Node 与后台节流
   * @returns {object} BrowserWindow 配置
   */
  options() {
    return { width: 1280, height: 800, show: false, useContentSize: true,
      webPreferences: { session: this.session, offscreen: true, backgroundThrottling: false,
        nodeIntegration: false, contextIsolation: true, sandbox: true, webSecurity: true, webviewTag: false } };
  }

  /**
   * 【桌面浏览器】【新建标签】创建独立页面并返回原生 CDP TargetID
   * @param {string} url 初始地址
   * @returns {string} 目标标识
   */
  create(url = 'about:blank') {
    if (!allowedPageUrl(url)) throw new Error('不允许的浏览器地址');
    const window = new BrowserWindow(this.options());
    const id = this.register(window);
    window.loadURL(url).catch(() => this.changed(id));
    return id;
  }

  /**
   * 【桌面浏览器】【注册页面】跟踪标题、导航、弹出页及销毁事件
   * @param {BrowserWindow} window 新页面窗口
   * @param {string} openerId 发起弹窗的目标标识
   * @returns {string} 原生 CDP TargetID
   */
  register(window, openerId = '') {
    const contents = window.webContents;
    const id = contents.getOrCreateDevToolsTargetId();
    this.tabs.set(id, { window, openerId });
    window.setMenu(null);
    contents.setUserAgent(this.userAgent);
    contents.setFrameRate(30);
    contents.on('page-title-updated', () => this.changed(id));
    contents.on('did-navigate', () => this.changed(id));
    contents.on('did-navigate-in-page', () => this.changed(id));
    contents.on('will-navigate', (event, url) => { if (!allowedPageUrl(url)) event.preventDefault(); });
    contents.on('will-redirect', (event, url) => { if (!allowedPageUrl(url)) event.preventDefault(); });
    contents.setWindowOpenHandler(({ url }) => allowedPageUrl(url)
      ? { action: 'allow', overrideBrowserWindowOptions: this.options() } : { action: 'deny' });
    contents.on('did-create-window', (child) => this.register(child, id));
    window.once('closed', () => {
      this.tabs.delete(id);
      this.emit('target', 'Target.targetDestroyed', { targetId: id });
    });
    this.emit('target', 'Target.targetCreated', { targetInfo: this.info(id) });
    return id;
  }

  /**
   * 【桌面浏览器】【目标信息】构造 Sai 依赖的 CDP 页面信息
   * @param {string} id 目标标识
   * @returns {object} TargetInfo
   */
  info(id) {
    const tab = this.tabs.get(id);
    if (!tab || tab.window.isDestroyed()) throw new Error('浏览器标签已关闭');
    return { targetId: id, type: 'page', title: tab.window.webContents.getTitle(),
      url: tab.window.webContents.getURL() || 'about:blank', attached: false,
      ...(tab.openerId ? { openerId: tab.openerId, canAccessOpener: true } : {}) };
  }

  /**
   * 【桌面浏览器】【状态通知】通知已连接的 Sai 更新地址和标题
   * @param {string} id 目标标识
   * @returns {void} 无返回值
   */
  changed(id) {
    const tab = this.tabs.get(id);
    if (tab && !tab.window.isDestroyed()) this.emit('target', 'Target.targetInfoChanged', { targetInfo: this.info(id) });
  }

  /**
   * 【桌面浏览器】【关闭标签】仅销毁指定浏览页面
   * @param {string} id 目标标识
   * @returns {boolean} 是否存在目标
   */
  close(id) {
    const tab = this.tabs.get(id);
    if (!tab) return false;
    tab.window.destroy();
    return true;
  }

  /**
   * 【桌面浏览器】【视口同步】同步原生离屏表面大小，使截图尺寸与 CDP 视口一致
   * @param {string} id 目标标识
   * @param {object} metrics CDP 视口参数
   * @returns {void} 无返回值
   */
  resize(id, metrics) {
    const window = this.tabs.get(id)?.window;
    const { width, height } = metrics;
    if (window && Number.isInteger(width) && Number.isInteger(height) && width > 0 && height > 0) {
      if (width > 8192 || height > 8192) throw new Error('浏览器视口超出上限');
      window.setContentSize(width, height);
    }
  }

  /**
   * 【桌面浏览器】【资源回收】销毁所有浏览页面
   * @returns {void} 无返回值
   */
  closeAll() { for (const id of [...this.tabs.keys()]) this.close(id); }
}

module.exports = { BrowserTabs, allowedPageUrl };
