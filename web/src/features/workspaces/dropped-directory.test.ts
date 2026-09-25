import { describe, expect, it } from "vitest";
import { parseDroppedDirectory } from "./dropped-directory";

describe("dropped-directory", () => {
  it("从 file URI 列表解析绝对路径", () => {
    expect(parseDroppedDirectory("file:///home/snemc/sai\n", "", "")).toEqual({
      path: "/home/snemc/sai",
      name: "sai"
    });
  });

  it("解码路径中的空格并忽略注释行", () => {
    expect(parseDroppedDirectory("# comment\r\nfile:///home/snemc/My%20App\r\n", "", "")).toEqual({
      path: "/home/snemc/My App",
      name: "My App"
    });
  });

  it("纯文本里的 file URI 和绝对路径同样可用", () => {
    expect(parseDroppedDirectory("", "file:///srv/work/web", "")).toEqual({ path: "/srv/work/web", name: "web" });
    expect(parseDroppedDirectory("", "/home/snemc/blog", "")).toEqual({ path: "/home/snemc/blog", name: "blog" });
    expect(parseDroppedDirectory("", "C:/Users/me/proj", "")).toEqual({ path: "C:/Users/me/proj", name: "proj" });
  });

  it("Windows 盘符 URI 去掉多余的前导斜杠", () => {
    expect(parseDroppedDirectory("file:///C:/Users/me/proj", "", "")).toEqual({
      path: "C:/Users/me/proj",
      name: "proj"
    });
  });

  it("浏览器只给文件名时不编造路径", () => {
    expect(parseDroppedDirectory("", "", "sai")).toEqual({ path: null, name: "sai" });
    expect(parseDroppedDirectory("", "随便一段文字", "")).toEqual({ path: null, name: null });
  });
});
