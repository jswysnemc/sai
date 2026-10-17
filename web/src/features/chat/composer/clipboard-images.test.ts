import { describe, expect, it } from "vitest";
import { imageFilesFromTransfer, isImageFile } from "./clipboard-images";

/**
 * 构造只暴露 files 与 items 的传输对象。
 *
 * @param files `files` 列表中的文件
 * @param itemFiles `items` 中以文件形式存在的内容
 * @returns 可供收集函数读取的传输对象
 */
function transfer(files: File[], itemFiles: File[]): DataTransfer {
  return {
    files,
    items: itemFiles.map((file) => ({
      kind: "file",
      getAsFile: () => file
    }))
  } as unknown as DataTransfer;
}

describe("clipboard images", () => {
  it("keeps every pasted image when files only contains the first one", () => {
    const first = new File([new Uint8Array([1])], "one.png", { type: "image/png" });
    const second = new File([new Uint8Array([2, 2])], "two.png", { type: "image/png" });

    const images = imageFilesFromTransfer(transfer([first], [first, second]));

    expect(images.map((file) => file.name)).toEqual(["one.png", "two.png"]);
  });

  it("accepts a windows clipboard image that has no mime type", () => {
    const file = new File([new Uint8Array([1])], "shot.png", { type: "" });

    expect(isImageFile(file)).toBe(true);
    expect(imageFilesFromTransfer(transfer([file], [])).map((item) => item.name)).toEqual(["shot.png"]);
  });

  it("ignores non-image files", () => {
    const note = new File([new Uint8Array([1])], "notes.txt", { type: "text/plain" });

    expect(isImageFile(note)).toBe(false);
    expect(imageFilesFromTransfer(transfer([note], [note]))).toEqual([]);
  });
});
