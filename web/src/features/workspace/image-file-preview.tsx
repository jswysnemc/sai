type ImageFilePreviewProps = {
  path: string;
};

/**
 * 在编辑器区域中自适应显示图像文件。
 *
 * @param props 图像相对路径
 * @returns 图像预览
 */
export function ImageFilePreview({ path }: ImageFilePreviewProps) {
  return (
    <div className="editor-image-preview">
      <img src={`/api/workspace/image?path=${encodeURIComponent(path)}`} alt={path.split(/[\\/]/).pop() ?? "图像预览"} />
    </div>
  );
}

/**
 * 判断路径是否属于浏览器可预览图像。
 *
 * @param path 文件路径
 * @returns 是否为图像扩展名
 */
export function isImageFile(path: string): boolean {
  return /\.(png|jpe?g|gif|webp|bmp|svg|ico)$/i.test(path);
}
