import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const outputDirectory = process.argv[2]
  ? resolve(process.argv[2])
  : fileURLToPath(new URL("../dist", import.meta.url));
const heavyChunk = /\/(?:monaco|mermaid(?:\.core)?|codemirror|terminal)-[^/]+\.js$/u;
const checks = [
  { entry: "index.html", name: "index", label: "登录页", maxBytes: 500_000 },
  { entry: "src/features/workspace/coding-page.tsx", name: "coding-page", label: "工作台", maxBytes: 2_000_000 }
];

/**
 * 收集入口及其静态依赖，保留动态导入的加载边界。
 * @param {Record<string, {file: string, imports?: string[]}>} manifest 生产构建清单
 * @param {string} entry 入口标识
 * @returns {string[]} 首次加载的文件列表
 */
function collectStaticFiles(manifest, entry) {
  const visited = new Set();
  const pending = [entry];
  while (pending.length > 0) {
    const key = pending.pop();
    if (visited.has(key)) continue;
    const chunk = manifest[key];
    if (!chunk) throw new Error(`构建清单缺少入口或依赖：${key}`);
    visited.add(key);
    pending.push(...(chunk.imports ?? []));
  }
  return [...visited].map((key) => manifest[key].file);
}

/**
 * 检查生产入口的依赖边界与脚本大小预算。
 * @returns {Promise<void>} 检查通过时完成，失败时抛出具体原因
 */
async function checkInitialBundles() {
  // 1. 【前端性能】【首屏检查】读取实际构建清单，避免仅检查源码导入形式
  const manifest = JSON.parse(await readFile(resolve(outputDirectory, ".vite/manifest.json"), "utf8"));
  const failures = [];
  for (const { entry, name, label, maxBytes } of checks) {
    const resolvedEntry = manifest[entry] ? entry
      : Object.keys(manifest).find((key) => manifest[key].isDynamicEntry && manifest[key].name === name);
    if (!resolvedEntry) {
      failures.push(`${label}没有独立的构建入口`);
      continue;
    }
    const files = collectStaticFiles(manifest, resolvedEntry);
    const sizes = await Promise.all(files.map((file) => stat(resolve(outputDirectory, file))));
    const bytes = sizes.reduce((total, info) => total + info.size, 0);
    const eagerLibraries = files.filter((file) => heavyChunk.test(file));
    console.info(`【前端性能】【首屏检查】${label}：${bytes} 字节，${files.length} 个脚本`);
    // 2. 【前端性能】【首屏检查】重型功能必须保持延迟加载，同时限制公共分包体积
    if (eagerLibraries.length > 0) failures.push(`${label}提前加载重型依赖：${eagerLibraries.join(", ")}`);
    if (bytes > maxBytes) failures.push(`${label}脚本 ${bytes} 字节，超过 ${maxBytes} 字节预算`);
  }
  if (failures.length > 0) throw new Error(failures.join("\n"));
}

checkInitialBundles().catch((error) => {
  console.error(`【前端性能】【首屏检查】${error.message}`);
  process.exitCode = 1;
});
