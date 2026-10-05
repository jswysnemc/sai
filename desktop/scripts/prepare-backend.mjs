import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, writeFile, access } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { parseArgs } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import targets from './build-targets.cjs';
import binaryTargets from './binary-target.cjs';
import { run } from './run-command.mjs';

const require = createRequire(import.meta.url);
const paths = require('./repo-paths.cjs');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const source = paths.sourceDir(root);
const { values } = parseArgs({ options: {
  platform: { type: 'string' }, arch: { type: 'string' }, reuse: { type: 'boolean' },
} });
const target = targets.resolveTarget(values);
const destination = paths.backendStagingDir(root, target.key);

// 1. 【桌面构建】【静态资源】先构建前端，确保 Rust 编译时嵌入当前 Web 工作台
if (!values.reuse) {
  targets.requireNativeHost(target);
  run('npm', ['ci'], path.join(source, 'web'));
  run('npm', ['run', 'build'], path.join(source, 'web'));
  // 2. 【桌面构建】【Windows 运行库】静态链接 CRT，避免目标系统缺少 VCRUNTIME140.dll
  const crt = target.platform === 'win'
    ? ['--config', 'target.x86_64-pc-windows-msvc.rustflags=["-C", "target-feature=+crt-static"]'] : [];
  run('cargo', ['build', '--release', '--locked', '--target', target.triple, ...crt, '-p', 'sai', '--bin', 'sai'], source);
}

// 3. 【桌面构建】【二进制收集】从 Cargo 元数据定位产物，写入独立暂存目录
const metadata = JSON.parse(run('cargo', ['metadata', '--no-deps', '--format-version', '1', '--locked'], source, true));
const name = target.binary;
let binary = path.resolve(process.env.SAI_BINARY_PATH || path.join(metadata.target_directory, target.triple, 'release', name));
if (values.reuse && !process.env.SAI_BINARY_PATH && target.key === targets.resolveTarget().key) {
  if (!await access(binary).then(() => true, () => false)) binary = path.join(metadata.target_directory, 'release', name);
}
const bytes = await readFile(binary);
binaryTargets.verifyBinaryTarget(bytes, target.key);
let version = `sai ${metadata.packages.find((pkg) => pkg.name === 'sai').version}`;
if (target.key === targets.resolveTarget().key) {
  version = run(binary, ['--version'], source, true);
  if (!/^sai \d+\.\d+\.\d+/i.test(version)) throw new Error('可执行文件不是有效的 Sai 后端');
  const help = run(binary, ['web', '--help'], source, true);
  for (const flag of ['--host', '--port', '--no-open', '--workspace']) {
    if (!help.includes(flag)) throw new Error(`Sai 后端不支持 ${flag}`);
  }
}
await mkdir(destination, { recursive: true });
await copyFile(binary, path.join(destination, name));
await chmod(path.join(destination, name), 0o755);
await copyFile(path.join(source, 'LICENSE'), path.join(destination, 'LICENSE'));

// 4. 【桌面构建】【产物记录】记录版本、源码状态及校验值，仅用于打包前验证
const commit = run('git', ['rev-parse', 'HEAD'], source, true);
const dirty = Boolean(run('git', ['status', '--porcelain'], source, true));
await writeFile(path.join(destination, 'manifest.json'), `${JSON.stringify({
  version, platform: target.platform, arch: target.arch, triple: target.triple, commit, dirty,
  reused: Boolean(values.reuse), builtAt: new Date().toISOString(),
  validation: target.key === targets.resolveTarget().key ? 'native-execution' : 'binary-header',
  sha256: createHash('sha256').update(bytes).digest('hex'),
}, null, 2)}\n`);
console.info(`【桌面构建】【准备完成】${destination}`);
