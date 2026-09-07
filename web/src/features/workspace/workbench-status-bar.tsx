import { Activity, Bot, GitBranch, Keyboard, SquareTerminal } from "lucide-react";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { WorkbenchWorkspacePath } from "./workbench-workspace-path";
import { useRuntimeActivity } from "../runtime-activity/use-runtime-activity";
import { requestWorkbenchCommand } from "./workbench-shortcuts";
import "./workbench-status-bar.css";

type WorkbenchStatusBarProps = { branch?: string; terminalOpen: boolean };

/**
 * 展示当前项目、分支和真实后台任务数量，并提供终端及命令入口。
 * @param props Git 分支与底部终端显隐状态
 * @returns 工作台底部状态栏
 */
export function WorkbenchStatusBar({ branch, terminalOpen }: WorkbenchStatusBarProps) {
  const { t } = useI18n();
  const activity = useRuntimeActivity();
  return (
    <footer className="workbench-status-bar" aria-label={t("Workspace status", "工作区状态")}>
      <div className="workbench-status-project">
        <WorkbenchWorkspacePath />
        {branch && <Button variant="ghost" size="small" className="workbench-status-branch" title={t(`Git branch: ${branch}`, `Git 分支：${branch}`)} onClick={() => requestWorkbenchCommand("open-changes")}><GitBranch size={12} /><span>{branch}</span></Button>}
      </div>
      <div className="workbench-status-actions">
        <Button variant="ghost" size="small" className="hidden md:inline-flex" onClick={() => window.dispatchEvent(new Event("sai:open-tasks"))} title={t("Background tasks", "后台任务")}>
          <Activity size={12} /><span>{t("Tasks", "任务")}</span><span className="workbench-status-count">{activity.runningTasks}</span>
        </Button>
        {activity.runningSubagents > 0 && <Button variant="ghost" size="small" onClick={() => window.dispatchEvent(new Event("sai:open-subagents"))} title={t("Running subagents", "运行中的子智能体")}><Bot size={12} /><span>{activity.runningSubagents}</span></Button>}
        <Button variant="ghost" size="small" aria-label={t("Toggle terminal", "切换终端")} aria-pressed={terminalOpen} onClick={() => requestWorkbenchCommand("toggle-terminal")}><SquareTerminal size={12} /><span className="hidden sm:inline">{t("Terminal", "终端")}</span></Button>
        <Button variant="ghost" size="small" aria-label={t("Open command menu", "打开命令菜单")} title={t("Commands and shortcuts", "命令与快捷键")} onClick={() => requestWorkbenchCommand("search")}><Keyboard size={13} /></Button>
      </div>
    </footer>
  );
}
