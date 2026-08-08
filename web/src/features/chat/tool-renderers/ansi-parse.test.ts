import { describe, expect, it } from "vitest";
import { hasAnsi, parseAnsi } from "./ansi-parse";

/** 转义序列起始字符，测试中同样避免直接嵌入不可见字符 */
const ESC = String.fromCharCode(27);

/** 组装一条 SGR 序列。 */
const sgr = (params: string) => `${ESC}[${params}m`;

describe("hasAnsi", () => {
  it("普通文本不含转义序列", () => {
    expect(hasAnsi("error: build failed [E0433]")).toBe(false);
  });

  it("含转义序列时返回 true", () => {
    expect(hasAnsi(`${sgr("31")}error`)).toBe(true);
  });
});

describe("parseAnsi", () => {
  it("纯文本返回单段无着色", () => {
    expect(parseAnsi("plain")).toEqual([{ text: "plain", color: "", bold: false, dim: false }]);
  });

  it("解析前景色并在重置后回到无色", () => {
    const source = `${sgr("31")}error${sgr("0")} done`;
    expect(parseAnsi(source)).toEqual([
      { text: "error", color: "red", bold: false, dim: false },
      { text: " done", color: "", bold: false, dim: false }
    ]);
  });

  it("同时解析加粗与颜色", () => {
    const source = `${sgr("1;32")}ok`;
    expect(parseAnsi(source)).toEqual([{ text: "ok", color: "green", bold: true, dim: false }]);
  });

  it("空参数序列等同于重置", () => {
    const source = `${sgr("31")}a${sgr("")}b`;
    expect(parseAnsi(source)).toEqual([
      { text: "a", color: "red", bold: false, dim: false },
      { text: "b", color: "", bold: false, dim: false }
    ]);
  });

  it("样式相同的相邻段落合并", () => {
    const source = `${sgr("31")}a${sgr("31")}b`;
    expect(parseAnsi(source)).toEqual([{ text: "ab", color: "red", bold: false, dim: false }]);
  });

  it("扩展色跳过色号参数不误读后续代码", () => {
    const source = `${sgr("38;5;208")}text`;
    expect(parseAnsi(source)).toEqual([{ text: "text", color: "", bold: false, dim: false }]);
  });

  it("剥离光标控制序列且不改变样式", () => {
    const source = `${sgr("32")}a${ESC}[2Kb`;
    expect(parseAnsi(source)).toEqual([{ text: "ab", color: "green", bold: false, dim: false }]);
  });

  it("剥离 OSC 标题序列", () => {
    const source = `${ESC}]0;window title${String.fromCharCode(7)}body`;
    expect(parseAnsi(source)).toEqual([{ text: "body", color: "", bold: false, dim: false }]);
  });

  it("明亮色使用独立语义名", () => {
    expect(parseAnsi(`${sgr("91")}x`)[0].color).toBe("bright-red");
  });

  it("代码 22 只取消加粗与变暗，保留颜色", () => {
    const source = `${sgr("1;31")}a${sgr("22")}b`;
    expect(parseAnsi(source)).toEqual([
      { text: "a", color: "red", bold: true, dim: false },
      { text: "b", color: "red", bold: false, dim: false }
    ]);
  });

  it("方括号字面量不被误判为转义序列", () => {
    expect(parseAnsi("error[E0433]: failed")).toEqual([
      { text: "error[E0433]: failed", color: "", bold: false, dim: false }
    ]);
  });
});
