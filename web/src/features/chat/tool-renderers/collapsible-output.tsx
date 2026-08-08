import { ChevronDown } from "lucide-react";
import { useMemo, useState } from "react";
import { AnsiOutput } from "./ansi-output";
import { useI18n } from "../../i18n/use-i18n";
import "./collapsible-output.css";

/** 超过这个行数的输出先折叠，展示前若干行 */
const COLLAPSE_THRESHOLD = 18;

/** 折叠状态下保留的行数 */
const VISIBLE_LINES = 12;

type CollapsibleOutputProps = {
  source: string;
  /**
   * 输出块的基础类名。
   *
   * 命令输出与通用工具输出各有一套内边距，两者都需要折叠能力，
   * 因此把版式类交给调用方决定，本组件只负责折叠与着色。
   */
  className?: string;
};

/**
 * 渲染命令输出，超长时折叠为前若干行并给出展开入口。
 *
 * 原先长输出交给一个固定高度的滚动区，一屏对话里会出现多个各自滚动的区域，
 * 滚轮落在哪个区域全看指针位置，长会话很难连续浏览。改为默认只展示开头，
 * 底部用渐隐说明"下面还有"，需要细看时再一次性展开。
 *
 * @param props source 为输出文本，className 为输出块类名
 * @returns 可展开的输出块
 */
export function CollapsibleOutput({ source, className = "shell-output" }: CollapsibleOutputProps) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  // 行数统计与切片是纯计算，计时器驱动的重渲染不应重跑
  const { visible, hiddenCount } = useMemo(() => splitOutput(source), [source]);

  if (hiddenCount === 0) {
    return <pre className={className}><code><AnsiOutput source={source} /></code></pre>;
  }

  return (
    <div className="collapsible-output">
      <pre className={expanded ? className : `${className} is-clipped`}>
        <code><AnsiOutput source={expanded ? source : visible} /></code>
      </pre>
      <button
        type="button"
        className="collapsible-output-toggle"
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
      >
        <ChevronDown size={12} className={expanded ? "rotate" : ""} aria-hidden />
        {expanded
          ? t("Collapse output", "收起输出")
          : t(`Show ${hiddenCount} more lines`, `展开剩余 ${hiddenCount} 行`)}
      </button>
    </div>
  );
}

/**
 * 将输出切分为折叠可见部分与隐藏行数。
 *
 * @param source 输出文本
 * @returns visible 为折叠时展示的文本，hiddenCount 为被折起的行数
 */
function splitOutput(source: string): { visible: string; hiddenCount: number } {
  const lines = source.split("\n");
  if (lines.length <= COLLAPSE_THRESHOLD) return { visible: source, hiddenCount: 0 };
  return {
    visible: lines.slice(0, VISIBLE_LINES).join("\n"),
    hiddenCount: lines.length - VISIBLE_LINES
  };
}
