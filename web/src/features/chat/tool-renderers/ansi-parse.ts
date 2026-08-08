/*
 * ANSI 转义序列解析
 *
 * 命令输出里的颜色信息原本被当作乱码直接渲染，SGR 序列以字面量形式
 * 混在正文里，既丢失了工具本来要表达的分级（错误红、路径青、提示灰），
 * 又让输出看起来是坏的。这里把 SGR 序列解析成分段，交给渲染层着色。
 *
 * 只处理 SGR（以 m 结尾的序列）。光标移动、清屏这类控制序列在
 * 一次性输出的场景没有意义，直接剥离，避免它们以字面量形式出现在正文里。
 *
 * ESC 一律由 String.fromCharCode(27) 构造：源码里直接嵌入不可见字符
 * 经不起复制粘贴、diff 与编辑器处理，改动几次就会静默失效。
 */

/** 一段颜色一致的输出文本 */
export type AnsiSegment = {
  text: string;
  /** 前景色语义名；无着色时为空 */
  color: string;
  bold: boolean;
  dim: boolean;
};

/** SGR 前景色代码到主题色语义名的映射，30-37 常规、90-97 明亮 */
const FOREGROUND: Record<number, string> = {
  30: "black",
  31: "red",
  32: "green",
  33: "yellow",
  34: "blue",
  35: "magenta",
  36: "cyan",
  37: "white",
  90: "bright-black",
  91: "bright-red",
  92: "bright-green",
  93: "bright-yellow",
  94: "bright-blue",
  95: "bright-magenta",
  96: "bright-cyan",
  97: "bright-white"
};

/** 转义序列起始字符 */
const ESC = String.fromCharCode(27);

/** 序列结束符 BEL，OSC 的两种终止方式之一 */
const BEL = String.fromCharCode(7);

/**
 * 转义序列匹配式。
 *
 * 两支分别对应 CSI（ESC [ 参数 结束字母）与 OSC（ESC ] 载荷 BEL 或 ESC \）。
 * 用 RegExp 构造而非字面量，ESC 才能以可见形式写进源码。
 *
 * @returns 带 g 标志的匹配式
 */
function ansiPattern(): RegExp {
  return new RegExp(`${ESC}\\[([0-9;]*)([A-Za-z])|${ESC}\\][^${BEL}${ESC}]*(?:${BEL}|${ESC}\\\\)?`, "g");
}

/**
 * 判断文本是否含 ANSI 转义序列。
 *
 * 渲染层据此决定走分段着色还是直接输出纯文本，
 * 绝大多数输出不含转义序列，跳过分段可以省掉一次遍历与大量 span 节点。
 *
 * @param value 待判断文本
 * @returns 含转义序列返回 true
 */
export function hasAnsi(value: string): boolean {
  return value.includes(ESC);
}

/**
 * 将含 ANSI 转义序列的文本解析为着色分段。
 *
 * @param value 原始输出文本
 * @returns 按颜色切分的文本段；无内容时返回空数组
 */
export function parseAnsi(value: string): AnsiSegment[] {
  const segments: AnsiSegment[] = [];
  const pattern = ansiPattern();
  let color = "";
  let bold = false;
  let dim = false;
  let cursor = 0;

  let match = pattern.exec(value);
  while (match !== null) {
    // 1. 先收下转义序列之前的正文，它沿用当前样式
    if (match.index > cursor) {
      pushSegment(segments, value.slice(cursor, match.index), color, bold, dim);
    }
    // 2. 只有 SGR 序列改变样式，其余控制序列剥离后不影响状态
    if (match[2] === "m") {
      const next = applySgr(match[1], color, bold, dim);
      color = next.color;
      bold = next.bold;
      dim = next.dim;
    }
    cursor = match.index + match[0].length;
    match = pattern.exec(value);
  }

  if (cursor < value.length) {
    pushSegment(segments, value.slice(cursor), color, bold, dim);
  }
  return segments;
}

/**
 * 按一条 SGR 参数串推进样式状态。
 *
 * @param params 分号分隔的 SGR 参数，空串等同于 0（重置）
 * @param color 当前前景色
 * @param bold 当前加粗状态
 * @param dim 当前变暗状态
 * @returns 应用参数后的样式状态
 */
function applySgr(
  params: string,
  color: string,
  bold: boolean,
  dim: boolean
): { color: string; bold: boolean; dim: boolean } {
  const codes = params === "" ? [0] : params.split(";").map((item) => Number(item) || 0);
  let nextColor = color;
  let nextBold = bold;
  let nextDim = dim;

  for (let index = 0; index < codes.length; index += 1) {
    const code = codes[index];
    if (code === 0) {
      nextColor = "";
      nextBold = false;
      nextDim = false;
    } else if (code === 1) {
      nextBold = true;
    } else if (code === 2) {
      nextDim = true;
    } else if (code === 22) {
      nextBold = false;
      nextDim = false;
    } else if (code === 39) {
      nextColor = "";
    } else if (code === 38) {
      // 扩展色：38;5;n 或 38;2;r;g;b。主题不提供 256 色对照，
      // 统一按未着色处理并跳过其参数，避免把色号当成后续 SGR 码解读
      index += codes[index + 1] === 5 ? 2 : 4;
      nextColor = "";
    } else if (FOREGROUND[code]) {
      nextColor = FOREGROUND[code];
    }
  }
  return { color: nextColor, bold: nextBold, dim: nextDim };
}

/**
 * 追加一段文本，与上一段样式相同时就地合并。
 *
 * 合并可观地减少 DOM 节点：逐字符换色的进度条输出经常产生成百上千段。
 *
 * @param segments 已收集的分段
 * @param text 待追加文本
 * @param color 前景色
 * @param bold 是否加粗
 * @param dim 是否变暗
 * @returns 无返回值
 */
function pushSegment(
  segments: AnsiSegment[],
  text: string,
  color: string,
  bold: boolean,
  dim: boolean
): void {
  if (!text) return;
  const last = segments.at(-1);
  if (last && last.color === color && last.bold === bold && last.dim === dim) {
    last.text += text;
    return;
  }
  segments.push({ text, color, bold, dim });
}
