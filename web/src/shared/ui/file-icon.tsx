import { Braces, File, FileCode, FileJson, FileTerminal, FileText, FileType, Image, Settings } from "lucide-react";
import type { LucideIcon } from "lucide-react";

type FileTypeIconProps = {
  name: string;
  size?: number;
};

type IconSpec = {
  icon: LucideIcon;
  color: string;
};

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg", "ico", "avif"]);

const EXTENSION_ICONS: Record<string, IconSpec> = {
  ts: { icon: FileCode, color: "#3178c6" },
  tsx: { icon: FileCode, color: "#3178c6" },
  mts: { icon: FileCode, color: "#3178c6" },
  js: { icon: FileCode, color: "#b7a032" },
  jsx: { icon: FileCode, color: "#b7a032" },
  cjs: { icon: FileCode, color: "#b7a032" },
  mjs: { icon: FileCode, color: "#b7a032" },
  rs: { icon: Braces, color: "#c47b52" },
  py: { icon: FileCode, color: "#3572a5" },
  go: { icon: FileCode, color: "#00add8" },
  java: { icon: FileCode, color: "#b07219" },
  json: { icon: FileJson, color: "#b58900" },
  yaml: { icon: FileType, color: "#a0788c" },
  yml: { icon: FileType, color: "#a0788c" },
  toml: { icon: Settings, color: "#9c4221" },
  ini: { icon: Settings, color: "#6e7781" },
  conf: { icon: Settings, color: "#6e7781" },
  md: { icon: FileText, color: "#4078c0" },
  txt: { icon: FileText, color: "#6e7781" },
  css: { icon: FileType, color: "#663399" },
  scss: { icon: FileType, color: "#c6538c" },
  html: { icon: FileCode, color: "#e34c26" },
  xml: { icon: FileCode, color: "#e34c26" },
  sh: { icon: FileTerminal, color: "#4eaa25" },
  bash: { icon: FileTerminal, color: "#4eaa25" },
  zsh: { icon: FileTerminal, color: "#4eaa25" },
  sql: { icon: FileCode, color: "#e38c00" },
  lock: { icon: Settings, color: "#8b949e" }
};

/**
 * 按文件扩展名渲染带品牌近似色的类型图标。
 *
 * @param props name 为文件名或路径，size 为图标尺寸（默认 14）
 * @returns 带内联颜色的 lucide 图标
 */
export function FileTypeIcon({ name, size = 14 }: FileTypeIconProps) {
  // 1. 提取文件扩展名
  const base = name.split("/").pop() ?? name;
  const extension = base.includes(".") ? base.split(".").pop()!.toLowerCase() : "";
  // 2. 图片类扩展名统一使用图片图标
  if (IMAGE_EXTENSIONS.has(extension)) return <Image size={size} style={{ color: "#c05299", flexShrink: 0 }} aria-hidden />;
  // 3. 命中映射时使用对应图标与颜色，否则使用通用文件图标
  const spec = EXTENSION_ICONS[extension];
  if (!spec) return <File size={size} style={{ color: "#8b949e", flexShrink: 0 }} aria-hidden />;
  const IconComponent = spec.icon;
  return <IconComponent size={size} style={{ color: spec.color, flexShrink: 0 }} aria-hidden />;
}
