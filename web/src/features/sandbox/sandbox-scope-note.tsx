import type { SandboxScope } from "../../api/contracts";
import { ShieldAlert, ShieldCheck } from "../../shared/ui/icons";
import { useI18n } from "../i18n/use-i18n";
import { sandboxScopeText } from "./sandbox-labels";
import "./sandbox-notes.css";

/**
 * 【沙箱】【审批卡】在权限卡中说明命令获批后是否留在沙箱内，并展示模型给出的提升理由。
 * @param props 沙箱范围
 * @returns 范围说明
 */
export function SandboxScopeNote({ scope }: { scope: SandboxScope }) {
  const { t } = useI18n();
  const { text, emphasis } = sandboxScopeText(scope, t);
  const Icon = emphasis ? ShieldAlert : ShieldCheck;
  return (
    <div className={emphasis ? "sandbox-note is-emphasis" : "sandbox-note"} data-sandbox-kind={scope.kind}>
      <Icon size={14} aria-hidden="true" />
      <div className="sandbox-note-body">
        <span>{text}</span>
        {scope.justification && (
          <span className="sandbox-note-detail">{t("Justification: ", "理由：")}{scope.justification}</span>
        )}
      </div>
    </div>
  );
}
