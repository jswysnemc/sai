/**
 * Material Icon Theme 的精简映射。
 *
 * 视觉资源来自 material-icon-theme 包（MIT），构建前由
 * scripts/copy-material-icons.mjs 按本文件引用的图标名裁剪复制到
 * public/material-icons/——映射即资源清单，新增条目后重跑脚本即可。
 */

/** 文件名精确匹配（小写） */
const FILE_NAMES: Record<string, string> = {
  "package.json": "nodejs",
  "pnpm-lock.yaml": "pnpm",
  "pnpm-workspace.yaml": "pnpm",
  "package-lock.json": "npm",
  "tsconfig.json": "tsconfig",
  "tsconfig.node.json": "tsconfig",
  "cargo.toml": "rust",
  "cargo.lock": "rust",
  "dockerfile": "docker",
  "docker-compose.yml": "docker",
  "docker-compose.yaml": "docker",
  ".gitignore": "git",
  ".gitattributes": "git",
  "readme.md": "readme",
  "license": "document",
  "license.md": "document",
  "eslint.config.js": "eslint",
  ".eslintrc.json": "eslint",
  "vite.config.ts": "vite",
  "vitest.config.ts": "vitest",
  "index.html": "html"
};

/**
 * 扩展名匹配。
 *
 * 用数组保证复合扩展名（test.ts、d.ts）在单段扩展名之前命中。
 */
const FILE_EXTENSIONS: Array<[string, string]> = [
  ["test.ts", "test-ts"],
  ["test.tsx", "test-jsx"],
  ["spec.ts", "test-ts"],
  ["spec.tsx", "test-jsx"],
  ["test.js", "test-js"],
  ["d.ts", "typescript-def"],
  ["ts", "typescript"],
  ["mts", "typescript"],
  ["cts", "typescript"],
  ["tsx", "react_ts"],
  ["js", "javascript"],
  ["mjs", "javascript"],
  ["cjs", "javascript"],
  ["jsx", "react"],
  ["rs", "rust"],
  ["py", "python"],
  ["go", "go"],
  ["java", "java"],
  ["c", "c"],
  ["h", "h"],
  ["cpp", "cpp"],
  ["json", "json"],
  ["jsonc", "json"],
  ["md", "markdown"],
  ["css", "css"],
  ["scss", "sass"],
  ["html", "html"],
  ["toml", "toml"],
  ["yaml", "yaml"],
  ["yml", "yaml"],
  ["sh", "console"],
  ["bash", "console"],
  ["zsh", "console"],
  ["fish", "console"],
  ["ps1", "powershell"],
  ["sql", "database"],
  ["svg", "svg"],
  ["png", "image"],
  ["jpg", "image"],
  ["jpeg", "image"],
  ["webp", "image"],
  ["gif", "image"],
  ["ico", "image"],
  ["avif", "image"],
  ["mp4", "video"],
  ["mp3", "audio"],
  ["woff", "font"],
  ["woff2", "font"],
  ["ttf", "font"],
  ["lock", "lock"],
  ["log", "log"],
  ["csv", "table"],
  ["xml", "xml"],
  ["pdf", "pdf"],
  ["zip", "zip"],
  ["txt", "document"],
  ["ini", "settings"],
  ["conf", "settings"],
  ["env", "settings"],
  ["vue", "vue"]
];

/** 目录名匹配（小写，不含 -open 后缀，展开态自动追加） */
const FOLDER_NAMES: Record<string, string> = {
  src: "folder-src",
  web: "folder-src",
  tests: "folder-test",
  test: "folder-test",
  __tests__: "folder-test",
  docs: "folder-docs",
  doc: "folder-docs",
  config: "folder-config",
  configs: "folder-config",
  scripts: "folder-scripts",
  assets: "folder-images",
  images: "folder-images",
  public: "folder-public",
  components: "folder-components",
  hooks: "folder-hook",
  utils: "folder-utils",
  api: "folder-api",
  dist: "folder-dist",
  build: "folder-dist",
  target: "folder-dist",
  node_modules: "folder-node",
  ".github": "folder-github",
  shared: "folder-shared",
  tools: "folder-tools",
  core: "folder-core",
  features: "folder-features"
};

/**
 * Markdown 语言标签到已有文件扩展名的别名。
 *
 * 只指向 FILE_EXTENSIONS / FILE_NAMES 里已经复制的图标，避免再引入一套资源。
 */
const LANGUAGE_FILES: Record<string, string> = {
  rust: "snippet.rs",
  rs: "snippet.rs",
  python: "snippet.py",
  py: "snippet.py",
  typescript: "snippet.ts",
  ts: "snippet.ts",
  javascript: "snippet.js",
  js: "snippet.js",
  tsx: "snippet.tsx",
  jsx: "snippet.jsx",
  golang: "snippet.go",
  go: "snippet.go",
  java: "snippet.java",
  c: "snippet.c",
  h: "snippet.h",
  cpp: "snippet.cpp",
  "c++": "snippet.cpp",
  cxx: "snippet.cpp",
  cc: "snippet.cpp",
  json: "snippet.json",
  jsonc: "snippet.json",
  markdown: "snippet.md",
  md: "snippet.md",
  css: "snippet.css",
  scss: "snippet.scss",
  sass: "snippet.scss",
  html: "snippet.html",
  toml: "snippet.toml",
  yaml: "snippet.yaml",
  yml: "snippet.yml",
  bash: "snippet.sh",
  sh: "snippet.sh",
  shell: "snippet.sh",
  zsh: "snippet.sh",
  console: "snippet.sh",
  powershell: "snippet.ps1",
  ps1: "snippet.ps1",
  sql: "snippet.sql",
  svg: "snippet.svg",
  xml: "snippet.xml",
  vue: "snippet.vue",
  text: "snippet.txt",
  plaintext: "snippet.txt",
  txt: "snippet.txt",
  dockerfile: "Dockerfile",
  docker: "Dockerfile"
};

const DEFAULT_FILE = "file";
const DEFAULT_FOLDER = "folder";
const DEFAULT_FOLDER_OPEN = "folder-open";

/**
 * 解析文件对应的图标名。
 *
 * 步骤:
 * 1. 文件名精确匹配
 * 2. 扩展名从长到短匹配（数组序即优先级）
 *
 * 参数:
 * - `name`: 文件名或路径
 *
 * 返回:
 * - material 图标名；未命中返回通用文件图标
 */
function resolveFileIcon(name: string): string {
  const base = (name.split("/").pop() ?? name).toLowerCase();
  const named = FILE_NAMES[base];
  if (named) return named;
  for (const [extension, icon] of FILE_EXTENSIONS) {
    if (base.endsWith(`.${extension}`)) return icon;
  }
  return DEFAULT_FILE;
}

/**
 * 解析目录对应的图标名，展开态使用 -open 变体。
 *
 * 参数:
 * - `name`: 目录名或路径
 * - `expanded`: 是否处于展开态
 *
 * 返回:
 * - material 图标名
 */
function resolveFolderIcon(name: string, expanded: boolean): string {
  const base = (name.replace(/\/+$/, "").split("/").pop() ?? name).toLowerCase();
  const icon = FOLDER_NAMES[base];
  if (icon) return expanded ? `${icon}-open` : icon;
  return expanded ? DEFAULT_FOLDER_OPEN : DEFAULT_FOLDER;
}

/**
 * 返回文件或目录的 Material 图标资源地址。
 *
 * 参数:
 * - `name`: 文件/目录名或路径
 * - `kind`: 条目类型
 * - `expanded`: 目录是否展开
 *
 * 返回:
 * - public 下的 SVG 地址
 */
/**
 * 把代码块语言标签换成能命中类型图标的文件名。
 *
 * 未知语言回落到通用文件图标。
 *
 * @param language Markdown 围栏语言，大小写不敏感
 * @returns 供文件图标解析使用的文件名
 */
export function fileNameForLanguage(language?: string): string {
  const key = (language ?? "").trim().toLowerCase().split(/[\s{]/)[0] ?? "";
  if (!key) return "snippet.txt";
  return LANGUAGE_FILES[key] ?? `snippet.${key}`;
}

export function materialIconUrl(name: string, kind: "file" | "directory", expanded = false): string {
  const icon = kind === "directory" ? resolveFolderIcon(name, expanded) : resolveFileIcon(name);
  return `${import.meta.env.BASE_URL}material-icons/${icon}.svg`;
}
