import { spawnSync } from 'node:child_process';

/**
 * 【桌面构建】【命令执行】兼容 pnpm 脚本版、独立二进制及 Windows 命令入口
 * @param {string} program 程序名称
 * @param {string[]} args 参数列表
 * @param {string} cwd 工作目录
 * @param {boolean} capture 是否捕获标准输出
 * @returns {string} 捕获的标准输出，失败时抛出错误
 */
/**
 * 【桌面构建】【命令执行】判断命令是否需要经 shell 启动
 * Windows 上 npm、pnpm、npx 是 .cmd 包装脚本，不经 shell 时 spawnSync 报 ENOENT
 * @param {string} program 程序名称
 * @param {string} platform 运行平台
 * @returns {boolean} 需要 shell 时为 true
 */
export function needsShell(program, platform) {
  return platform === 'win32' && ['npm', 'pnpm', 'npx'].includes(program);
}

export function run(program, args, cwd, capture = false) {
  const pnpmScript = program === 'pnpm' && /\.[cm]?js$/.test(process.env.npm_execpath || '')
    ? process.env.npm_execpath : null;
  const result = spawnSync(pnpmScript ? process.execPath : program,
    pnpmScript ? [pnpmScript, ...args] : args, {
      cwd, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
      stdio: capture ? ['ignore', 'pipe', 'inherit'] : 'inherit',
      shell: !pnpmScript && needsShell(program, process.platform),
    });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${program} 执行失败，退出码 ${result.status}`);
  return result.stdout?.trim() || '';
}
