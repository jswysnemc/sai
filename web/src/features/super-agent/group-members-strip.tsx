import { useState } from "react";
import { Bot, CircleStop, Cpu, MessageSquare, Plus } from "../../shared/ui/icons";
import type { SpawnRunnerPayload, SuperGroupMember } from "../../api/super-agent-contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";

type GroupMembersStripProps = {
  members: SuperGroupMember[];
  spawning: boolean;
  onMentionMember: (name: string) => void;
  onSpawnRunner: (payload: SpawnRunnerPayload) => Promise<void>;
  onMessageRunner: (runnerId: string, message: string) => Promise<void>;
  onStopRunner: (runnerId: string) => Promise<void>;
};

/**
 * 【超级智能体】【成员状态栏】展示群组 Leader 与全部 Runner 状态，支持快速 @提及、创建 Runner、私信追问与停止操作。
 *
 * @param props 成员列表及操作回调
 * @returns 成员状态条
 */
export function GroupMembersStrip({
  members,
  spawning,
  onMentionMember,
  onSpawnRunner,
  onMessageRunner,
  onStopRunner
}: GroupMembersStripProps) {
  const { t } = useI18n();
  const [spawnOpen, setSpawnOpen] = useState(false);
  const [runnerName, setRunnerName] = useState("worker-1");
  const [specialty, setSpecialty] = useState("engineering");
  const [taskTitle, setTaskTitle] = useState("");
  const [prompt, setPrompt] = useState("");
  const [msgTarget, setMsgTarget] = useState<SuperGroupMember | null>(null);
  const [directMsg, setDirectMsg] = useState("");

  const handleSpawn = async () => {
    if (!runnerName.trim() || !prompt.trim()) return;
    await onSpawnRunner({
      name: runnerName.trim(),
      specialty: specialty.trim(),
      task_title: taskTitle.trim() || undefined,
      prompt: prompt.trim()
    });
    setPrompt("");
    setTaskTitle("");
    setSpawnOpen(false);
  };

  const handleDirectSend = async () => {
    if (!msgTarget || !directMsg.trim()) return;
    await onMessageRunner(msgTarget.id, directMsg.trim());
    setDirectMsg("");
    setMsgTarget(null);
  };

  return (
    <div className="sa-members-strip">
      {members.map((member) => {
        const isLeader = member.role === "leader";
        const isLive = member.status === "running" || member.status === "waiting" || member.status === "idle";
        return (
          <div key={member.id} className="sa-member-chip">
            <button
              type="button"
              className="sa-member-chip-btn"
              title={member.last_activity || member.model_label}
              onClick={() => onMentionMember(member.name)}
            >
              <i className={`sa-dot ${member.status}`} />
              {isLeader ? <Bot size={14} /> : <Cpu size={14} />}
              <strong>{member.name}</strong>
              <span className="sa-badge">{isLeader ? "Leader" : member.status}</span>
            </button>
            {!isLeader && (
              <>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={t("Message runner", "向 Runner 发消息")}
                  title={t("Message runner", "向 Runner 发消息")}
                  onClick={() => setMsgTarget(member)}
                >
                  <MessageSquare size={12} />
                </Button>
                {isLive && (
                  <Button
                    size="icon"
                    variant="ghost-danger"
                    aria-label={t("Stop runner", "停止 Runner")}
                    title={t("Stop runner", "停止 Runner")}
                    onClick={() => void onStopRunner(member.id)}
                  >
                    <CircleStop size={12} />
                  </Button>
                )}
              </>
            )}
          </div>
        );
      })}

      <Button size="small" variant="secondary" onClick={() => setSpawnOpen(true)}>
        <Plus size={14} />
        {t("Spawn Runner", "新建 Runner")}
      </Button>

      <Modal
        open={spawnOpen}
        title={t("Spawn asynchronous Runner", "创建并启动异步 Runner")}
        description={t(
          "The Runner executes in an isolated context and submits a structured report to the Leader upon completion.",
          "Runner 将在独立上下文中异步执行任务，完成后自动向 Leader 提交结构化工作报告。"
        )}
        size="medium"
        onClose={() => setSpawnOpen(false)}
        footer={
          <>
            <Button variant="secondary" onClick={() => setSpawnOpen(false)}>
              {t("Cancel", "取消")}
            </Button>
            <Button
              variant="primary"
              disabled={spawning || !runnerName.trim() || !prompt.trim()}
              onClick={() => void handleSpawn()}
            >
              {spawning ? t("Starting...", "启动中...") : t("Launch Runner", "启动 Runner")}
            </Button>
          </>
        }
      >
        <div className="sa-form-grid">
          <label className="sa-field">
            <span>{t("Runner name", "Runner 名称")}</span>
            <input className="sa-input" value={runnerName} onChange={(e) => setRunnerName(e.target.value)} />
          </label>
          <label className="sa-field">
            <span>{t("Specialty", "专长角色")}</span>
            <input className="sa-input" value={specialty} onChange={(e) => setSpecialty(e.target.value)} />
          </label>
          <label className="sa-field" style={{ gridColumn: "1 / -1" }}>
            <span>{t("Task title", "任务标题")}</span>
            <input
              className="sa-input"
              value={taskTitle}
              onChange={(e) => setTaskTitle(e.target.value)}
              placeholder={t("Optional concise title", "可选简短任务标题")}
            />
          </label>
          <label className="sa-field" style={{ gridColumn: "1 / -1" }}>
            <span>{t("Task instructions", "具体任务指令")}</span>
            <textarea
              className="sa-textarea"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder={t("Describe what the Runner should investigate, build, or verify...", "填写 Runner 需要执行的代码修改、调研或验证目标...")}
            />
          </label>
        </div>
      </Modal>

      <Modal
        open={Boolean(msgTarget)}
        title={msgTarget ? `${t("Message Runner", "向 Runner 发送指令")}: @${msgTarget.name}` : ""}
        size="small"
        onClose={() => setMsgTarget(null)}
        footer={
          <>
            <Button variant="secondary" onClick={() => setMsgTarget(null)}>
              {t("Cancel", "取消")}
            </Button>
            <Button variant="primary" disabled={!directMsg.trim()} onClick={() => void handleDirectSend()}>
              {t("Send", "发送")}
            </Button>
          </>
        }
      >
        <label className="sa-field">
          <span>{t("Follow-up instruction", "补充说明或追问")}</span>
          <textarea
            className="sa-textarea"
            value={directMsg}
            onChange={(e) => setDirectMsg(e.target.value)}
            placeholder={t("Injected directly at the next step boundary...", "将在下一步间隙注入到 Runner 上下文中...")}
          />
        </label>
      </Modal>
    </div>
  );
}
