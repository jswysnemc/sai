import { readFile, readdir, rm, writeFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { gzip } from "node:zlib";

const compress = promisify(gzip);
const textExtensions = new Set([".js", ".mjs", ".css", ".html", ".json", ".svg"]);
const outputDirectory = process.argv[2]
  ? resolve(process.argv[2])
  : fileURLToPath(new URL("../dist", import.meta.url));

/**
 * 收集可以压缩的文本构建产物。
 * @param {string} directory 构建输出目录
 * @returns {Promise<string[]>} 文本资源的完整路径
 */
async function collectAssets(directory) {
  const paths = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) paths.push(...await collectAssets(path));
    else if (entry.isFile() && textExtensions.has(extname(entry.name))) paths.push(path);
  }
  return paths;
}

/**
 * 为单个资源写入确定性的 gzip 表示，只保留体积更小的结果。
 * @param {string} path 原始资源路径
 * @returns {Promise<number>} 节省的字节数
 */
async function compressAsset(path) {
  const original = await readFile(path);
  const encoded = await compress(original, { level: 9 });
  if (encoded.length >= original.length) {
    await rm(`${path}.gz`, { force: true });
    return 0;
  }
  await writeFile(`${path}.gz`, encoded);
  return original.length - encoded.length;
}

/**
 * 【静态资源】【构建压缩】限制并发生成压缩资源，供内置服务直接返回。
 * @returns {Promise<void>} 压缩产物写入完成后的结果
 */
async function compressAssets() {
  const paths = await collectAssets(outputDirectory);
  let savedBytes = 0;
  // 1. 【静态资源】【构建压缩】分批处理文件，控制大型编辑器资源的内存占用
  for (let offset = 0; offset < paths.length; offset += 8) {
    const savings = await Promise.all(paths.slice(offset, offset + 8).map(compressAsset));
    savedBytes += savings.reduce((total, bytes) => total + bytes, 0);
  }
  console.info(`【静态资源】【构建压缩】检查 ${paths.length} 个文本资源，压缩后共减少 ${savedBytes} 字节`);
}

compressAssets().catch((error) => {
  console.error(`【静态资源】【构建压缩】${error.message}`);
  process.exitCode = 1;
});
