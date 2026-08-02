import { describe, expect, it } from "vitest";
import {
  appendEditableImages,
  isEditorDirty,
  isEditorSubmittable,
  toEditableImages
} from "./user-message-editor-model";

describe("user-message-editor-model", () => {
  it("把原消息图片带入编辑态，重发时不丢图", () => {
    const images = toEditableImages(["data:image/png;base64,AA==", "data:image/png;base64,BB=="]);

    expect(images.map((image) => image.dataUrl)).toEqual([
      "data:image/png;base64,AA==",
      "data:image/png;base64,BB=="
    ]);
    // 标识在编辑期内唯一，删除中间一张不影响其余项
    expect(new Set(images.map((image) => image.id)).size).toBe(2);
  });

  it("图片原样保留时不算改动", () => {
    const initial = ["data:image/png;base64,AA=="];

    expect(isEditorDirty("原文", toEditableImages(initial), "原文", initial)).toBe(false);
  });

  it("删除图片计为改动", () => {
    const initial = ["data:image/png;base64,AA=="];

    expect(isEditorDirty("原文", [], "原文", initial)).toBe(true);
  });

  it("正文与图片同时为空时不可提交", () => {
    expect(isEditorSubmittable("   ", [])).toBe(false);
    expect(isEditorSubmittable("", toEditableImages(["data:image/png;base64,AA=="]))).toBe(true);
    expect(isEditorSubmittable("文字", [])).toBe(true);
  });

  it("追加图片时标识不与既有项冲突", () => {
    const existing = toEditableImages(["data:image/png;base64,AA=="]);

    const next = appendEditableImages(existing, ["data:image/png;base64,BB=="], 1);

    expect(next.images).toHaveLength(2);
    expect(next.images[1].id).toBe(2);
    expect(next.sequence).toBe(2);
  });
});
