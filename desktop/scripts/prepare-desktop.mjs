import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { parseArgs } from 'node:util';
import { mkdir, readFile, copyFile, chmod, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import targets from './build-targets.cjs';
import binaryTargets from './binary-target.cjs';
import { run } from './run-command.mjs';

const require = createRequire(import.meta.url);
const paths = require('./repo-paths.cjs');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const { values } = parseArgs({ options: { platform: { type: 'string' }, arch: { type: 'string' } } });
const target = targets.resolveTarget(values);
// 1. 【桌面构建】【标题栏】先构建独立渲染页，不写入安装包输出目录
run('pnpm', ['exec', 'vite', 'build', '--config', 'renderer/vite.config.mjs'], root);
const project = path.join(root, 'native/browser-launcher');
let binary = process.env.SAI_DESKTOP_LAUNCHER_BINARY;
if (!binary) {
  // 2. 【桌面构建】【启动器】在对应主机编译浏览器桥接程序
  targets.requireNativeHost(target);
  const crt = target.platform === 'win'
    ? ['--config', 'target.x86_64-pc-windows-msvc.rustflags=["-C", "target-feature=+crt-static"]'] : [];
  run('cargo', ['build', '--release', '--locked', '--target', target.triple, ...crt], project);
  const metadata = JSON.parse(run('cargo', ['metadata', '--no-deps', '--format-version', '1'], project, true));
  binary = path.join(metadata.target_directory, target.triple, 'release',
    target.platform === 'win' ? 'sai-browser-launcher.exe' : 'sai-browser-launcher');
}
const bytes = await readFile(binary);
binaryTargets.verifyBinaryTarget(bytes, target.key);
// 3. 【桌面构建】【启动器暂存】写入独立暂存目录，不进入 dist 安装包输出
const directory = paths.hostStagingDir(root, target.key);
await mkdir(directory, { recursive: true });
const name = target.platform === 'win' ? 'sai-browser-launcher.exe' : 'sai-browser-launcher';
await copyFile(binary, path.join(directory, name));
await chmod(path.join(directory, name), 0o755);
await writeFile(path.join(directory, 'manifest.json'), `${JSON.stringify({
  platform: target.platform, arch: target.arch, sha256: createHash('sha256').update(bytes).digest('hex'),
}, null, 2)}\n`);
console.info(`【桌面构建】【外壳准备】${target.key}`);
