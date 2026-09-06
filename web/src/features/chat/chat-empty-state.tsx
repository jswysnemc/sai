import { FileSearch, GitPullRequest, ListChecks } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { FOCUS_COMPOSER_EVENT } from "./composer/composer-events";
import "./chat-empty-state.css";

type ChatEmptyStateProps = { children: ReactNode; onChoose: (prompt: string) => void; disabled: boolean };

/**
 * 展示新会话入口，任务建议只填入草稿，由用户确认发送。
 * @param props 输入区域、草稿更新回调和会话可用状态
 * @returns 紧凑的新会话界面
 */
export function ChatEmptyState({ children, onChoose, disabled }: ChatEmptyStateProps) {
  const { t } = useI18n();
  const suggestions = [
    { label: t("Explore this project", "了解项目"), icon: FileSearch, prompt: t("Explain this project's architecture and show me the key entry points.", "请梳理当前项目的架构、主要模块和关键入口。") },
    { label: t("Review my changes", "审阅变更"), icon: GitPullRequest, prompt: t("Review my uncommitted changes for bugs, regressions, and missing validation.", "请审阅当前尚未提交的修改，检查错误、回归风险和验证缺口。") },
    { label: t("Plan a feature", "规划功能"), icon: ListChecks, prompt: t("Help me plan a feature for this project. First understand the existing implementation and ask what I want to build.", "请帮我规划一个新功能，先了解现有实现，再询问我希望完成的具体需求。") }
  ];
  return (
    <div className="empty-session-stage">
      <div className="empty-session-greeting">
        <h2>{t("What are we working on?", "今天要完成什么？")}</h2>
        <p>{t("Plan, build, and review in your workspace.", "在当前项目中规划、开发和审阅代码。")}</p>
      </div>
      {children}
      <div className="empty-session-suggestions" aria-label={t("Suggested tasks", "任务建议")}>
        {suggestions.map(({ label, icon: Icon, prompt }) => <Button variant="ghost" key={label} disabled={disabled} onClick={() => {
          onChoose(prompt);
          window.requestAnimationFrame(() => window.dispatchEvent(new Event(FOCUS_COMPOSER_EVENT)));
        }}><Icon size={14} /><span>{label}</span></Button>)}
      </div>
    </div>
  );
}
