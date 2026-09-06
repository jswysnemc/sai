import { MessagesSquare, Route } from "lucide-react";
import { SegmentedControl } from "../../shared/ui/segmented-control";
import { useI18n } from "../i18n/use-i18n";

export type ChatView = "conversation" | "trajectory";

/**
 * 在会话内容与请求轨迹之间切换视图。
 * @param props 当前视图及切换回调
 * @returns 会话视图分段控件
 */
export function ChatViewSwitch({ view, onChange }: { view: ChatView; onChange: (view: ChatView) => void }) {
  const { t } = useI18n();
  return <SegmentedControl className="chat-view-switch" value={view} onChange={onChange} ariaLabel={t("Session view", "会话视图")} options={[
    { value: "conversation", label: t("Chat", "对话"), icon: <MessagesSquare size={13} aria-hidden /> },
    { value: "trajectory", label: t("Trajectory", "轨迹"), icon: <Route size={13} aria-hidden /> }
  ]} />;
}
