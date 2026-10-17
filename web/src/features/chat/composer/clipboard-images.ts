const IMAGE_EXTENSION = /\.(png|jpe?g|gif|webp|bmp|svg|avif|heic|heif)$/i;

/**
 * 判断文件是否应按图片附件处理。
 *
 * Windows 剪贴板里的图片经常没有 MIME，只保留扩展名。
 *
 * @param file 待判断的文件
 * @returns 是否为图片
 */
export function isImageFile(file: File): boolean {
  if (file.type.startsWith("image/")) return true;
  return IMAGE_EXTENSION.test(file.name);
}

/**
 * 从一次粘贴或拖放里收集全部图片。
 *
 * Chromium 往往只把第一张放进 `files`，其余留在 `items`。
 * 必须在事件处理函数里同步读取，异步之后剪贴板数据会失效。
 *
 * @param data 剪贴板或拖放数据
 * @returns 去重后的图片文件，顺序与剪贴板一致
 */
export function imageFilesFromTransfer(data: DataTransfer): File[] {
  const seen = new Set<string>();
  const images: File[] = [];
  const push = (file: File | null) => {
    if (!file || !isImageFile(file)) return;
    const key = `${file.name}\0${file.size}\0${file.lastModified}\0${file.type}`;
    if (seen.has(key)) return;
    seen.add(key);
    images.push(file);
  };
  // 1. 先收 files，保留系统给出的顺序
  Array.from(data.files).forEach(push);
  // 2. 再收 items，补上 files 里被丢掉的后续图片
  Array.from(data.items).forEach((item) => {
    if (item.kind !== "file") return;
    push(item.getAsFile());
  });
  return images;
}
