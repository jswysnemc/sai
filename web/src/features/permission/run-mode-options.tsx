import { Hand, NotepadText, ShieldAlert, ShieldCheck } from "../../shared/ui/icons";
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
        "Ask before changing files; commands run in the sandbox.",
        "改文件前先问我；命令在沙箱内执行。"
      ),
      icon: <span className="run-mode-icon audit"><Hand size={16} /></span>
    },
    {
      value: "auto_audit",
      label: t("Auto audit", "自动审核"),
      description: t(
        "Auto-review changes; sandbox escapes are judged strictly.",
        "自动审核变更；离开沙箱的命令从严判断。"
      ),
      icon: <span className="run-mode-icon auto"><ShieldCheck size={16} /></span>
    },
    {
      value: "plan",
      label: t("Plan mode", "计划模式"),
      description: t(
        "Plan first; commands run read-only.",
        "编辑前先出计划；命令只读执行。"
      ),
      icon: <span className="run-mode-icon plan"><NotepadText size={16} /></span>
    },
    {
      value: "yolo",
      label: t("Full access", "完全访问"),
      description: t(
        "Minimize prompts; no sandbox.",
        "减少确认次数；不经过沙箱。"
      ),
      icon: <span className="run-mode-icon yolo"><ShieldAlert size={16} /></span>
    }
  ];
}
