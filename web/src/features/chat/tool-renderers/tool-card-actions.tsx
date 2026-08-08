import { Check, ClipboardList, Copy } from "lucide-react";
import type { MouseEvent } from "react";
import { useCopyAction } from "./use-copy-action";
import { useI18n } from "../../i18n/use-i18n";
import "./tool-card-actions.css";

type ToolCardActionsProps = {
  /** 可复制的调用参数，通常是命令或路径 */
  target: string;
  /** 可复制的工具输出 */
  output: string;
};

/**
 * 渲染工具卡头部的次级操作。
 *
 * 常态隐藏、悬停或键盘聚焦时出现：复制命令与复制输出都是低频操作，
 * 常驻会让每一行工具卡都多出两个图标，把折叠行本该表达的信息挤掉。
 *
 * @param props target 为待复制的调用参数，output 为待复制的输出
 * @returns 操作按钮组；两者都为空时返回 null
 */
export function ToolCardActions({ target, output }: ToolCardActionsProps) {
  const { t } = useI18n();
  const targetCopy = useCopyAction();
  const outputCopy = useCopyAction();
  if (!target && !output) return null;

  /**
   * 执行复制且不触发卡片展开。
   *
   * @param event 按钮点击事件
   * @param run 实际的复制动作
   * @returns 无返回值
   */
  const handle = (event: MouseEvent<HTMLButtonElement>, run: () => void) => {
    event.stopPropagation();
    run();
  };

  return (
    <span className="tool-card-actions">
      {target && (
        <button
          type="button"
          className={targetCopy.copied ? "is-copied" : undefined}
          onClick={(event) => handle(event, () => targetCopy.copy(target))}
          title={t("Copy input", "复制调用内容")}
          aria-label={t("Copy input", "复制调用内容")}
        >
          {targetCopy.copied ? <Check size={12} /> : <Copy size={12} />}
        </button>
      )}
      {output && (
        <button
          type="button"
          className={outputCopy.copied ? "is-copied" : undefined}
          onClick={(event) => handle(event, () => outputCopy.copy(output))}
          title={t("Copy output", "复制输出")}
          aria-label={t("Copy output", "复制输出")}
        >
          {outputCopy.copied ? <Check size={12} /> : <ClipboardList size={12} />}
        </button>
      )}
    </span>
  );
}
