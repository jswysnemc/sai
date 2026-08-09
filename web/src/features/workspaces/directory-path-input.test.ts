import { describe, expect, it } from "vitest";
import {
  directoryOfInput,
  ensureTrailingSlash,
  filterOfInput,
  lastSegmentOf,
  stripTrailingSlash
} from "./directory-path-input";

describe("directory path input parsing", () => {
  it("以斜杠结尾的输入是目录本身，过滤词为空", () => {
    expect(directoryOfInput("/home/")).toBe("/home/");
    expect(filterOfInput("/home/")).toBe("");
  });

  it("末段作为过滤词，前缀作为浏览目录", () => {
    expect(directoryOfInput("/home/sn")).toBe("/home/");
    expect(filterOfInput("/home/sn")).toBe("sn");
  });

  it("无分隔符的输入不产生跳转目录", () => {
    expect(directoryOfInput("abc")).toBe("");
    expect(filterOfInput("abc")).toBe("abc");
  });

  it("反斜杠输入按正斜杠归一", () => {
    expect(directoryOfInput("C:\\Users\\sn")).toBe("C:/Users/");
    expect(filterOfInput("C:\\Users\\sn")).toBe("sn");
  });

  it("尾斜杠补齐与剥离保留文件系统根", () => {
    expect(ensureTrailingSlash("/home")).toBe("/home/");
    expect(stripTrailingSlash("/home/")).toBe("/home");
    expect(stripTrailingSlash("/")).toBe("/");
    expect(stripTrailingSlash("C:/")).toBe("C:/");
  });

  it("提取目录形态路径的最后一段", () => {
    expect(lastSegmentOf("/home/snemc/")).toBe("snemc");
    expect(lastSegmentOf("/")).toBe("");
  });
});
