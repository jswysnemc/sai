import { Hand, NotepadText, ShieldAlert, ShieldCheck } from "lucide-react";
import type { RunMode } from "../../api/contracts";
import type { SelectOption } from "../../shared/ui/select/select";
import type { Translate } from "../i18n/i18n-context";
import "./run-mode-options.css";

/**
 * 构造统一的运行模式选项。
 *
 * @param t 国际化翻译方法
 * @returns 带文案、说明和区分图标的运行模式选项
 */
export function createRunModeOptions(t: Translate): SelectOption<RunMode>[] {
  return [
    {
      value: "audited",
      label: t("Confirm changes", "变更前确认"),
      description: t(
        "Ask before changing files.",
        "改文件前先问我。"
      ),
      icon: <span className="run-mode-icon audit"><Hand size={16} /></span>
    },
    {
      value: "auto_audit",
      label: t("Auto audit", "自动审核"),
      description: t(
        "Automatically audit file changes.",
        "自动审核文件变更。"
      ),
      icon: <span className="run-mode-icon auto"><ShieldCheck size={16} /></span>
    },
    {
      value: "plan",
      label: t("Plan mode", "计划模式"),
      description: t(
        "Make a plan before editing.",
        "编辑前先出计划。"
      ),
      icon: <span className="run-mode-icon plan"><NotepadText size={16} /></span>
    },
    {
      value: "yolo",
      label: t("Full access", "完全访问"),
      description: t(
        "Minimize confirmation prompts.",
        "减少确认次数。"
      ),
      icon: <span className="run-mode-icon yolo"><ShieldAlert size={16} /></span>
    }
  ];
}
