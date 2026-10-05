import path from 'node:path';
import { createRequire } from 'node:module';
import { parseArgs } from 'node:util';
import { fileURLToPath } from 'node:url';
import targets from './build-targets.cjs';
import { run } from './run-command.mjs';
import { checkSaiContracts } from './check-sai-contracts.mjs';

const require = createRequire(import.meta.url);
const paths = require('./repo-paths.cjs');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const { values } = parseArgs({ options: {
  platform: { type: 'string' }, arch: { type: 'string' },
  reuse: { type: 'boolean' }, dir: { type: 'boolean' }, 'skip-backend': { type: 'boolean' },
} });
const target = targets.resolveTarget(values);
await checkSaiContracts(paths.sourceDir(root));
run(process.execPath, [path.join(root, 'scripts/prepare-desktop.mjs'), '--platform', target.platform, '--arch', target.arch], root);

// 1. 【桌面构建】【后端准备】源码构建检查主机，复用时由文件头确认实际平台
if (!values['skip-backend']) {
  if (!values.reuse) targets.requireNativeHost(target);
  run(process.execPath, [path.join(root, 'scripts', 'prepare-backend.mjs'),
    '--platform', target.platform, '--arch', target.arch, ...(values.reuse ? ['--reuse'] : [])], root);
}

// 2. 【桌面构建】【安装包生成】显式传入平台和架构，只输出安装包文件
run('pnpm', ['exec', 'electron-builder', `--${target.platform}`, `--${target.arch}`,
  ...(values.dir ? ['--dir'] : []), '--publish', 'never'], root);
if (!values.dir) run(process.execPath, [path.join(root, 'scripts', 'write-checksums.mjs')], root);
