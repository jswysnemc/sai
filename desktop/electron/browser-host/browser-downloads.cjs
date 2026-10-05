const { randomUUID } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { EventEmitter } = require('node:events');

/** 【桌面浏览器】【下载适配】将 Electron 下载事件转换成 Sai 现有的 CDP 事件 */
class BrowserDownloads extends EventEmitter {
  /**
   * 【桌面浏览器】【下载监听】监听浏览器专属会话
   * @param {Session} session Electron 浏览器会话
   * @returns {BrowserDownloads} 下载管理器
   */
  constructor(session) {
    super();
    this.configuration = { behavior: 'deny', eventsEnabled: false };
    session.on('will-download', (_event, item, contents) => this.begin(item, contents));
  }

  /**
   * 【桌面浏览器】【下载策略】设置 Sai 提供的保存目录，禁用原生保存对话框
   * @param {object} configuration CDP Browser.setDownloadBehavior 参数
   * @returns {void} 无返回值
   */
  configure(configuration) {
    if (!['deny', 'allow', 'allowAndName'].includes(configuration.behavior)) throw new Error('不支持的下载策略');
    if (configuration.behavior !== 'deny') {
      if (!path.isAbsolute(configuration.downloadPath || '')) throw new Error('下载目录必须为绝对路径');
      fs.mkdirSync(configuration.downloadPath, { recursive: true });
    }
    this.configuration = { ...configuration };
  }

  /**
   * 【桌面浏览器】【下载开始】保存 GUID 文件并转发进度，保持 Sai 下载接口兼容
   * @param {DownloadItem} item Electron 下载对象
   * @param {WebContents} contents 发起下载的页面
   * @returns {void} 无返回值
   */
  begin(item, contents) {
    const configuration = { ...this.configuration };
    if (configuration.behavior === 'deny') { item.cancel(); return; }
    const guid = randomUUID();
    const filename = path.basename(item.getFilename()).replace(/[\\/\x00-\x1f]/g, '_') || 'download';
    item.setSavePath(path.join(configuration.downloadPath, configuration.behavior === 'allowAndName' ? guid : filename));
    const notify = (method, params) => {
      if (configuration.eventsEnabled) this.emit('event', method, params);
    };
    notify('Browser.downloadWillBegin', { guid, url: item.getURL(), suggestedFilename: filename,
      frameId: contents?.getOrCreateDevToolsTargetId() || '' });
    const progress = (state) => notify('Browser.downloadProgress', {
      guid, state, receivedBytes: item.getReceivedBytes(), totalBytes: item.getTotalBytes(),
    });
    item.on('updated', () => progress('inProgress'));
    item.once('done', (_event, state) => progress(state === 'completed' ? 'completed' : 'canceled'));
  }
}

module.exports = { BrowserDownloads };
