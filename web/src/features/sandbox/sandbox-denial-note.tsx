import type { SandboxDenial } from "../../api/contracts";
import { ShieldAlert } from "../../shared/ui/icons";
import { useI18n } from "../i18n/use-i18n";
import "./sandbox-notes.css";

/**
 * 【沙箱】【命令结果】说明命令被沙箱拦截的类型、证据行与下一步做法。
 * @param props 拦截说明
 * @returns 拦截提示
 */
export function SandboxDenialNote({ denial }: { denial: SandboxDenial }) {
  const { t } = useI18n();
  const kind = denial.kind === "network" ? t("network", "网络") : t("filesystem", "文件系统");
  return (
    <div className="sandbox-note is-emphasis sandbox-denial-note" role="status">
      <ShieldAlert size={14} aria-hidden="true" />
      <div className="sandbox-note-body">
        <span>{t("Blocked by sandbox", "沙箱拦截")} · {kind}</span>
        {denial.hint && <span className="sandbox-note-detail">{denial.hint}</span>}
        {denial.evidence && <code>{denial.evidence}</code>}
      </div>
    </div>
  );
}
