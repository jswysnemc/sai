import { Bot } from "lucide-react";
import type { ToolLifecycle } from "../run-event-reducer";
import type { Translate } from "../../i18n/i18n-context";
import { useI18n } from "../../i18n/use-i18n";
import type { CodexSubagentActivity, CodexSubagentActivityKind } from "./codex-subagent-data";
import { ToolCardShell } from "./tool-card-shell";
import { ToolStatusMark, toneOfState } from "./tool-icon";
import "./codex-subagent-tool-view.css";

type CodexSubagentToolViewProps = {
  tool: ToolLifecycle;
  activity: CodexSubagentActivity;
  expanded: boolean;
  onToggle: () => void;
};

/**
 * 渲染 Codex 原生子智能体活动工具卡。
 *
 * @param props 工具生命周期、子智能体活动与展开控制
 * @returns 子智能体名称、线程、活动和调用状态视图
 */
export function CodexSubagentToolView({
  tool,
  activity,
  expanded,
  onToggle
}: CodexSubagentToolViewProps) {
  const { t } = useI18n();
  return (
    <ToolCardShell
      tone={toneOfState(tool.status)}
      icon={<Bot size={14} />}
      title={t("Subagent", "子智能体")}
      target={activity.name}
      targetTitle={activity.path}
      meta={<span className={`codex-subagent-activity is-${activity.activity}`}>{activityLabel(activity.activity, t)}</span>}
      status={<ToolStatusMark state={tool.status} />}
      expanded={expanded}
      onToggle={onToggle}
      className="codex-subagent-shell"
    >
      <div className="codex-subagent-tool-view">
        <dl>
          <div><dt>{t("Agent", "智能体")}</dt><dd>{activity.name}</dd></div>
          <div><dt>{t("Thread", "线程")}</dt><dd><code>{activity.threadId}</code></dd></div>
          <div><dt>{t("Path", "路径")}</dt><dd><code>{activity.path}</code></dd></div>
          <div><dt>{t("Activity", "活动")}</dt><dd>{activityLabel(activity.activity, t)}</dd></div>
          <div><dt>{t("Tool lifecycle", "调用状态")}</dt><dd>{toolStatusLabel(tool.status, t)}</dd></div>
        </dl>
        {tool.output && (
          <section className="codex-subagent-output">
            <span>{t("Result", "结果")}</span>
            <pre>{tool.output}</pre>
          </section>
        )}
      </div>
    </ToolCardShell>
  );
}

/**
 * 返回 Codex 子智能体活动的界面文案。
 *
 * @param activity 活动类型
 * @param t 双语文本选择方法
 * @returns 本地化活动名称
 */
function activityLabel(activity: CodexSubagentActivityKind, t: Translate): string {
  const labels: Record<CodexSubagentActivityKind, string> = {
    started: t("Started", "已启动"),
    interacted: t("Interacted", "已交互"),
    interrupted: t("Interrupted", "已中断")
  };
  return labels[activity];
}

/**
 * 返回工具生命周期的界面文案。
 *
 * @param status 当前工具状态
 * @param t 双语文本选择方法
 * @returns 本地化状态名称
 */
function toolStatusLabel(status: ToolLifecycle["status"], t: Translate): string {
  const labels: Record<ToolLifecycle["status"], string> = {
    preparing: t("Preparing", "准备中"),
    running: t("Running", "进行中"),
    completed: t("Completed", "已完成"),
    failed: t("Failed", "失败")
  };
  return labels[status];
}
