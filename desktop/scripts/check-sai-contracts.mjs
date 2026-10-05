import path from 'node:path';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const paths = require('./repo-paths.cjs');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/**
 * 【桌面适配】【升级检查】检查 Sai 已有扩展接口，接口移动或移除时明确中止构建
 * @param {string} source Sai 源码目录
 * @returns {Promise<void>} 所有接口存在时完成；不验证接口的运行语义
 */
export async function checkSaiContracts(source) {
  const contracts = [
    ['src/browser/launcher.rs', ['SAI_BROWSER_EXECUTABLE', 'remote-debugging-port', 'user-data-dir']],
    ['src/browser/profile.rs', ['SAI_BROWSER_PROFILE', 'DevToolsActivePort']],
    ['src/browser/session_lifecycle.rs', ['Target.setDiscoverTargets', 'Browser.close']],
    ['web/src/features/workspace/workspace-panel-options.ts', ['sai:open-workspace-panel']],
    ['web/src/features/workspace/workspace-layout.tsx', ['OPEN_WORKSPACE_PANEL_EVENT', 'detail?.tab']],
    ['web/src/features/theme/theme.ts', ['dataset.theme']],
    ['web/src/shared/styles/tokens/neutral-themes.css', ['--paper:', '--ink:', '--ink-soft:', '--line:']],
    ['web/src/shared/styles/tokens/workbench.css', ['--sidebar-surface:']],
  ];
  const results = await Promise.allSettled(contracts.map(async ([file, symbols]) => {
    const content = await readFile(path.join(source, file), 'utf8');
    for (const symbol of symbols) {
      if (!content.includes(symbol)) throw new Error(`${file} 缺少 ${symbol}`);
    }
  }));
  const failures = results.filter((result) => result.status === 'rejected').map((result) => result.reason.message);
  if (failures.length) throw new Error(`Sai 桌面接口发生变化，请更新适配层并执行集成测试：\n${failures.join('\n')}`);
  console.info('【桌面适配】【升级检查】浏览器入口、面板事件及主题变量均存在');
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await checkSaiContracts(paths.sourceDir(root));
}
