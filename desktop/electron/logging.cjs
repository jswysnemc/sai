const fs = require('node:fs');
const path = require('node:path');

/**
 * 【桌面端】【日志脱敏】移除启动令牌、认证头和会话 Cookie
 * @param {string} value 原始日志
 * @returns {string} 脱敏后的日志
 */
function redact(value) {
  return String(value)
    .replace(/([?&]token=)[^\s&#]+/gi, '$1[redacted]')
    .replace(/(Bearer\s+)[^\s]+/gi, '$1[redacted]')
    .replace(/(sai_web_session=)[^;\s]+/gi, '$1[redacted]');
}

/**
 * 【桌面端】【日志记录】创建有大小限制的日志文件，失败时回退到标准错误
 * @param {string} directory 日志保存目录
 * @returns {{file: string, info: Function, error: Function}} 日志接口
 */
function createLogger(directory) {
  fs.mkdirSync(directory, { recursive: true });
  const file = path.join(directory, 'desktop.log');
  /**
   * 【桌面端】【日志写入】轮换并追加一条脱敏记录
   * @param {string} level 日志级别
   * @param {string} message 消息内容
   * @returns {void} 无返回值
   */
  function write(level, message) {
    const line = `${new Date().toISOString()} ${level} ${redact(message)}\n`;
    try {
      if (fs.existsSync(file) && fs.statSync(file).size > 2 * 1024 * 1024) {
        fs.rmSync(`${file}.1`, { force: true });
        fs.renameSync(file, `${file}.1`);
      }
      fs.appendFileSync(file, line, { mode: 0o600 });
    } catch {
      process.stderr.write(line);
    }
  }
  return { file, info: write.bind(null, 'INFO'), error: write.bind(null, 'ERROR') };
}

module.exports = { createLogger, redact };
