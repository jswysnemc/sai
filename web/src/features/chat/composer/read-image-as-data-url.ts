/**
 * 将图片文件读取为 data URL。
 *
 * 输入区草稿与用户消息编辑器都需要把本地文件转成可直接提交的 data URL，
 * 逻辑相同，因此抽到这里共用。
 *
 * @param file 图片文件
 * @param onError 读取失败时用于构造错误信息的文案，缺省时使用英文兜底
 * @returns 图片 data URL
 */
export function readImageAsDataUrl(file: File, onError?: () => Error): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? onError?.() ?? new Error("Failed to read image"));
    reader.readAsDataURL(file);
  });
}
