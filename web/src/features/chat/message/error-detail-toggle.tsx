import { ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";
import { useI18n } from "../../i18n/use-i18n";
import "./run-error-notice.css";

type ErrorDetailToggleProps = {
  detail: string;
  /** 为 true 时只渲染切换钮，详情正文由父级在下方承接（避免动作栏里再套一块） */
  toggleOnly?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
};

/**
 * 渲染可展开的错误详情。
 *
 * 默认自包含；在运行错误卡片里用受控 + toggleOnly，把切换钮放进标题行、
 * 详情铺在卡片底部，避免「粉框套灰框」。
 *
 * @param props 详情文本与可选受控状态
 * @returns 切换控件，或切换控件 + 详情正文
 */
export function ErrorDetailToggle({ detail, toggleOnly = false, open, onOpenChange }: ErrorDetailToggleProps) {
  const { t } = useI18n();
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false);
  const resolvedOpen = open ?? uncontrolledOpen;
  const setOpen = (value: boolean) => {
    onOpenChange?.(value);
    if (open === undefined) setUncontrolledOpen(value);
  };
  if (!detail.trim()) return null;

  const toggle = (
    <button
      type="button"
      className="run-error-detail-toggle"
      aria-expanded={resolvedOpen}
      onClick={() => setOpen(!resolvedOpen)}
    >
      {resolvedOpen ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
      <span>{resolvedOpen ? t("Hide details", "收起详情") : t("Details", "详情")}</span>
    </button>
  );

  if (toggleOnly) return toggle;

  return (
    <div className={`run-error-detail${resolvedOpen ? " is-open" : ""}`}>
      {toggle}
      {resolvedOpen ? <pre className="run-error-detail-body">{detail}</pre> : null}
    </div>
  );
}
