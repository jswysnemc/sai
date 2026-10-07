import { ChevronRight, FileText } from "../../../shared/ui/icons";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import { usePersistedExpand } from "./tool-expand-state";
import { useI18n } from "../../i18n/use-i18n";
import type { ToolPart } from "./group-activity-parts";

/**
 * 连续已完成读取的紧凑折叠组。
 *
 * 默认收起，只显示「读取 N」；展开后逐条列出工具卡。
 *
 * @param props 已完成的读类工具部件
 * @returns 可展开的读取折叠组
 */
export function ReadFold({ parts }: { parts: ToolPart[] }) {
  const { t } = useI18n();
  const id = `read-fold-${parts[0]?.id ?? "empty"}-${parts[parts.length - 1]?.id ?? "empty"}`;
  const [open, setOpen] = usePersistedExpand(id, false);
  const label = t(`${parts.length} reads`, `读取 ${parts.length}`);
  return (
    <div className={`read-fold${open ? " is-open" : ""}`}>
      <button
        type="button"
        className="activity-preamble-head"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="activity-preamble-icon" aria-hidden><FileText size={14} /></span>
        <span className="activity-preamble-label">{label}</span>
        <ChevronRight size={14} />
      </button>
      {open ? (
        <div className="read-fold-body">
          {parts.map((part) => <ToolLifecycleCard key={part.id} tool={part.tool} />)}
        </div>
      ) : null}
    </div>
  );
}
