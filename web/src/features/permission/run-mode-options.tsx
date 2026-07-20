import { Bot, ClipboardList, ShieldCheck, Zap } from "lucide-react";
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
      value: "yolo",
      label: "YOLO",
      description: t(
        "Execute allowed tools without permission prompts.",
        "不询问工具权限，直接执行允许的工具。"
      ),
      icon: <span className="run-mode-icon yolo"><Zap size={13} /></span>
    },
    {
      value: "audited",
      label: t("Audit", "审计"),
      description: t(
        "Ask before write tools and restrict them to the workspace sandbox.",
        "写入工具逐次询问，并限制在工作区沙盒内。"
      ),
      icon: <span className="run-mode-icon audit"><ShieldCheck size={13} /></span>
    },
    {
      value: "auto_audit",
      label: t("Auto audit", "自动审核"),
      description: t(
        "LLM auto-review runs in parallel with human approval; human decision wins if first.",
        "LLM 自动审核与人工审核并行，人工先决定则优先生效。"
      ),
      icon: <span className="run-mode-icon auto"><Bot size={13} /></span>
    },
    {
      value: "plan",
      label: t("Plan", "规划"),
      description: t(
        "Allow read-only tools only; modifications and write operations are prohibited.",
        "仅允许只读工具，禁止修改文件和执行写操作。"
      ),
      icon: <span className="run-mode-icon plan"><ClipboardList size={13} /></span>
    }
  ];
}
