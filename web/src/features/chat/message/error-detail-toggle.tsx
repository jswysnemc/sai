import { ChevronDown, ChevronUp } from "lucide-react";
import { useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";

/**
 * 渲染可展开的模型错误详情。
 *
 * @param props 错误详情文本
 * @returns 详情切换按钮与按需显示的原始错误
 */
export function ErrorDetailToggle({ detail }: { detail: string }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  if (!detail.trim()) return null;
  return (
    <div className="run-error-detail">
      <Button
        className="run-error-detail-toggle"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        {open ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
        <span>{open ? t("Hide error details", "收起错误详情") : t("View error details", "查看错误详情")}</span>
      </Button>
      {open && <pre>{detail}</pre>}
    </div>
  );
}
