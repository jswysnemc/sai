/** 编辑态里的一张图片，既可能来自原消息，也可能是新加入的。 */
export type EditableImage = {
  /** 编辑期内稳定的本地标识 */
  id: number;
  dataUrl: string;
};

/**
 * 把原消息图片地址转换为带本地标识的编辑态图片。
 *
 * @param imageUrls 原消息图片地址
 * @returns 编辑态图片列表
 */
export function toEditableImages(imageUrls: string[]): EditableImage[] {
  return imageUrls.map((dataUrl, index) => ({ id: index, dataUrl }));
}

/**
 * 判断编辑内容相对原消息是否有改动。
 *
 * 未改动时按钮仍可点击，因为重发本身就是有意义的操作；此判定仅用于界面提示。
 *
 * @param content 当前正文
 * @param images 当前图片
 * @param initialContent 原消息正文
 * @param initialImageUrls 原消息图片地址
 * @returns 有改动时返回 true
 */
export function isEditorDirty(
  content: string,
  images: EditableImage[],
  initialContent: string,
  initialImageUrls: string[]
): boolean {
  if (content !== initialContent) return true;
  if (images.length !== initialImageUrls.length) return true;
  return images.some((image, index) => image.dataUrl !== initialImageUrls[index]);
}

/**
 * 判断当前编辑内容能否提交。
 *
 * 正文与图片同时为空时无内容可发，须禁用提交。
 *
 * @param content 当前正文
 * @param images 当前图片
 * @returns 可提交时返回 true
 */
export function isEditorSubmittable(content: string, images: EditableImage[]): boolean {
  return Boolean(content.trim()) || images.length > 0;
}

/**
 * 按当前序号追加一批图片。
 *
 * @param images 现有图片
 * @param dataUrls 新增图片的 data URL
 * @param sequence 当前已发放的最大标识
 * @returns 追加后的图片列表与新的标识游标
 */
export function appendEditableImages(
  images: EditableImage[],
  dataUrls: string[],
  sequence: number
): { images: EditableImage[]; sequence: number } {
  return {
    images: [
      ...images,
      ...dataUrls.map((dataUrl, offset) => ({ id: sequence + offset + 1, dataUrl }))
    ],
    sequence: sequence + dataUrls.length
  };
}
