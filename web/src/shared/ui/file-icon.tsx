import { materialIconUrl } from "./material-icons";

type FileTypeIconProps = {
  name: string;
  size?: number;
};

/**
 * 按文件名渲染 Material 风格类型图标。
 *
 * 图标资源由 material-icon-theme 按映射清单裁剪复制（见
 * shared/ui/material-icons.ts）；未命中映射时回落通用文件图标，
 * 资源加载失败时隐藏占位不留裂图。
 *
 * @param props name 为文件名或路径，size 为图标尺寸（默认 14）
 * @returns 文件类型图标
 */
export function FileTypeIcon({ name, size = 14 }: FileTypeIconProps) {
  return (
    <img
      src={materialIconUrl(name, "file")}
      alt=""
      width={size}
      height={size}
      loading="lazy"
      decoding="async"
      draggable={false}
      style={{ flexShrink: 0 }}
      aria-hidden
      onError={(event) => {
        event.currentTarget.style.visibility = "hidden";
      }}
    />
  );
}

/**
 * 按目录名渲染 Material 风格目录图标，展开态使用 open 变体。
 *
 * @param props name 为目录名或路径，expanded 表示展开态，size 为图标尺寸
 * @returns 目录图标
 */
export function DirectoryIcon({ name, expanded = false, size = 14 }: FileTypeIconProps & { expanded?: boolean }) {
  return (
    <img
      src={materialIconUrl(name, "directory", expanded)}
      alt=""
      width={size}
      height={size}
      loading="lazy"
      decoding="async"
      draggable={false}
      style={{ flexShrink: 0 }}
      aria-hidden
      onError={(event) => {
        event.currentTarget.style.visibility = "hidden";
      }}
    />
  );
}
