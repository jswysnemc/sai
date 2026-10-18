const path = require('node:path');

/**
 * 【桌面端】【工作区参数】从启动参数里取出要打开的目录。
 *
 * Windows 目录右键会把目标目录追加到可执行文件参数中。开发模式下前面还有
 * Electron 自身和本应用目录，这两项不能当成工作区。
 *
 * @param {string[]} argv 进程参数
 * @param {{ execPath: string, appPath: string, isDirectory: (candidate: string) => boolean }} context 可执行文件、应用目录和目录判断
 * @returns {string|null} 绝对路径；没有目录参数时返回 null
 */
function workspaceFromLaunch(argv, { execPath, appPath, isDirectory }) {
  const ignored = new Set([path.resolve(execPath), path.resolve(appPath)]);
  for (let index = argv.length - 1; index >= 0; index -= 1) {
    const arg = argv[index];
    if (!arg || arg.startsWith('-')) continue;
    const resolved = path.resolve(arg);
    if (ignored.has(resolved) || !isDirectory(resolved)) continue;
    return resolved;
  }
  return null;
}

module.exports = { workspaceFromLaunch };
