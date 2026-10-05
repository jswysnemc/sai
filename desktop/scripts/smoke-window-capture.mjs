import { spawnSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';

/**
 * 【桌面测试】【窗口截图】捕获自有 X11 窗口，包含标题栏和所有 WebContentsView
 * @param {ElectronApplication} application Playwright 应用对象
 * @param {string} destination 截图输出路径
 * @returns {Promise<void>} 截图失败时抛出错误
 */
export async function captureWindow(application, destination) {
  // 1. 【桌面测试】【绘制等待】等待合成器提交主题和窗口尺寸变化，避免截取上一帧
  await delay(300);
  const id = await application.evaluate(({ BrowserWindow }) =>
    `0x${BrowserWindow.getAllWindows()[0].getNativeWindowHandle().readUInt32LE(0).toString(16)}`);
  const args = ['-window', id, destination];
  let result = spawnSync('magick', ['import', ...args], { encoding: 'utf8' });
  if (result.error?.code === 'ENOENT') result = spawnSync('import', args, { encoding: 'utf8' });
  if (result.error || result.status !== 0) throw new Error(`窗口截图失败：${result.error?.message || result.stderr}`);
}
