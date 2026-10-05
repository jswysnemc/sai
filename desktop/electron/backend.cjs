const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const { setTimeout: delay } = require('node:timers/promises');
const path = require('node:path');
const { parseStartupUrl, checkHealth } = require('./backend-protocol.cjs');

/**
 * 【桌面端】【后端路径】解析开发目录或安装包内的后端路径
 * @param {object} options 打包状态、资源目录和项目目录
 * @returns {string} Sai 可执行文件的绝对路径
 */
function backendPath({ packaged, resourcesPath, projectRoot }) {
  const name = process.platform === 'win32' ? 'sai.exe' : 'sai';
  const platform = process.platform === 'win32' ? 'win' : process.platform === 'darwin' ? 'mac' : 'linux';
  return packaged
    ? path.join(resourcesPath, 'backend', name)
    : path.join(projectRoot, '.staging', `${platform}-${process.arch}`, 'backend', name);
}

/** 【桌面端】【后端生命周期】管理一个独立的 Sai 服务进程 */
class Backend {
  /**
   * 【桌面端】【后端配置】保存启动参数和错误回调
   * @param {object} options 可执行路径、工作目录、日志、退出回调和可选测试参数
   * @returns {Backend} 后端管理实例
   */
  constructor({ executable, cwd, workspace, logger, onExit = () => {}, env = process.env, prefixArgs = [], timeout = 30000, port = 0 }) {
    Object.assign(this, { executable, cwd, workspace, logger, onExit, env, prefixArgs, timeout, port });
    this.child = null;
    this.stopping = false;
    this.ready = false;
    this.failure = null;
  }

  /**
   * 【桌面端】【服务启动】等待启动地址及健康检查成功，不复用其他服务
   * @returns {Promise<URL>} 当前进程的服务地址，含一次性启动令牌
   */
  async start() {
    if (this.child) throw new Error('后端已启动');
    // 1. 【桌面端】【进程创建】由操作系统分配端口，保持默认认证行为
    this.child = spawn(this.executable, [
      ...this.prefixArgs, 'web', '--host', '127.0.0.1', '--port', String(this.port), '--no-open',
      ...(this.workspace ? ['--workspace', this.workspace] : []),
    ], {
      cwd: this.cwd, env: this.env, stdio: ['ignore', 'pipe', 'pipe'],
      windowsHide: true, detached: process.platform !== 'win32',
    });
    this.closed = new Promise((resolve) => {
      this.child.once('error', (error) => { this.failure = error; });
      this.child.once('close', (code, signal) => {
        this.failure ??= new Error(`Sai 后端已退出（${signal || code}）`);
        resolve();
        if (this.ready && !this.stopping) this.onExit(this.failure);
      });
    });
    let url;
    const output = createInterface({ input: this.child.stdout });
    output.on('line', (line) => {
      url ??= parseStartupUrl(line);
      // 2. 【桌面端】【日志边界】不持久化会话正文和请求参数，仅记录启动及退出状态
      if (line.startsWith('Sai Web:')) this.logger.info(`【桌面端】【后端启动】${line}`);
    });
    const errors = createInterface({ input: this.child.stderr });
    errors.on('line', (line) => this.logger.error(`【桌面端】【后端错误】${line.slice(0, 4096)}`));
    const deadline = Date.now() + this.timeout;
    try {
      // 3. 【桌面端】【服务就绪】同时检查进程状态和 HTTP 响应，处理启动失败及取消
      while (Date.now() < deadline) {
        if (this.failure) throw this.failure;
        if (this.stopping) throw new Error('后端启动已取消');
        if (url && await checkHealth(url.origin)) {
          if (this.failure) throw this.failure;
          this.ready = true;
          this.logger.info(`【桌面端】【服务就绪】pid=${this.child.pid} origin=${url.origin}`);
          return url;
        }
        await delay(100);
      }
      throw new Error('Sai 后端在 30 秒内未就绪');
    } catch (error) {
      await this.stop();
      throw error;
    }
  }

  /**
   * 【桌面端】【服务停止】限时等待正常退出，超时终止所属进程组
   * @returns {Promise<void>} 后端退出后完成，可重复调用
   */
  stop() {
    this.stopping = true;
    this.stopPromise ??= this.shutdown();
    return this.stopPromise;
  }

  /**
   * 【桌面端】【进程回收】Unix 发送 SIGTERM，Windows 回收进程树
   * @returns {Promise<void>} 进程已回收或抛出清理错误
   */
  async shutdown() {
    const child = this.child;
    if (!child || child.exitCode !== null || child.signalCode !== null || !child.pid) return;
    if (process.platform === 'win32') {
      await new Promise((resolve, reject) => {
        const task = spawn('taskkill', ['/pid', String(child.pid), '/T', '/F'], { windowsHide: true });
        task.once('error', reject);
        task.once('close', resolve);
      });
    } else {
      child.kill('SIGTERM');
    }
    const finished = await Promise.race([this.closed.then(() => true), delay(5000, false)]);
    if (!finished) {
      this.logger.error('【桌面端】【强制退出】后端未及时停止，终止进程组');
      try {
        if (process.platform === 'win32') child.kill();
        else process.kill(-child.pid, 'SIGKILL');
      } catch (error) {
        if (error.code !== 'ESRCH') throw error;
      }
      await this.closed;
    }
    this.logger.info('【桌面端】【服务停止】后端已退出');
  }
}

module.exports = { Backend, backendPath };
