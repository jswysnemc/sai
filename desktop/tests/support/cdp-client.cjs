const { WebSocket } = require('ws');
const { EventEmitter } = require('node:events');

/** 【桌面测试】【CDP 客户端】供浏览器协议集成测试发送命令与接收事件 */
class CdpClient extends EventEmitter {
  /**
   * 【桌面测试】【连接创建】连接测试浏览器端点
   * @param {string} url WebSocket 地址
   * @returns {CdpClient} 协议客户端
   */
  constructor(url) {
    super();
    this.socket = new WebSocket(url);
    this.pending = new Map();
    this.nextId = 0;
    this.ready = new Promise((resolve, reject) => {
      this.socket.once('open', resolve);
      this.socket.once('error', reject);
    });
    this.socket.on('message', (raw) => {
      const message = JSON.parse(raw);
      const pending = this.pending.get(message.id);
      if (pending) {
        this.pending.delete(message.id);
        clearTimeout(pending.timer);
        message.error ? pending.reject(new Error(message.error.message)) : pending.resolve(message.result);
      } else if (message.method) this.emit(message.method, message.params, message.sessionId);
    });
    this.socket.on('close', () => {
      for (const pending of this.pending.values()) { clearTimeout(pending.timer); pending.reject(new Error('CDP closed')); }
      this.pending.clear();
    });
  }

  /**
   * 【桌面测试】【命令请求】发送带可选会话的 CDP 命令
   * @param {string} method 方法名
   * @param {object} params 方法参数
   * @param {string} sessionId 可选页面会话
   * @returns {Promise<object>} 协议结果
   */
  async send(method, params = {}, sessionId) {
    await this.ready;
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 15000);
      this.pending.set(id, { resolve, reject, timer });
      this.socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }

  /** 【桌面测试】【连接关闭】释放客户端；无参数，无返回值 */
  close() { this.socket.close(); }
}

module.exports = { CdpClient };
