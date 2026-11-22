import { useState } from "react";
import { Plus, Trash2 } from "../../shared/ui/icons";
import type { SuperGroupSummary } from "../../api/super-agent-contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../i18n/use-i18n";

type GroupSidebarProps = {
  groups: SuperGroupSummary[];
  activeGroupId?: string;
  creating: boolean;
  onSelectGroup: (groupId: string) => void;
  onCreateGroup: (payload: { name: string; description: string; leader_name: string }) => Promise<void>;
  onDeleteGroup: (groupId: string) => Promise<void>;
};

/**
 * 【超级智能体】【群组侧栏】展示全部协作群组、Leader 与 Runner 运行计数，并提供新建与删除操作。
 *
 * @param props 群组列表及增删选择回调
 * @returns 群组侧栏组件
 */
export function GroupSidebar({
  groups,
  activeGroupId,
  creating,
  onSelectGroup,
  onCreateGroup,
  onDeleteGroup
}: GroupSidebarProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const [modalOpen, setModalOpen] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [leaderName, setLeaderName] = useState("Leader");

  const handleSubmit = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    await onCreateGroup({
      name: trimmed,
      description: description.trim(),
      leader_name: leaderName.trim() || "Leader"
    });
    setName("");
    setDescription("");
    setLeaderName("Leader");
    setModalOpen(false);
  };

  const handleDelete = async (event: React.MouseEvent, item: SuperGroupSummary) => {
    event.stopPropagation();
    const ok = await confirm({
      title: t("Delete group", "删除群组"),
      description: t(
        `Delete "${item.group.name}" and stop all active Runners in this group?`,
        `确认删除群组「${item.group.name}」并停止其中全部运行中的 Runner？`
      ),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (ok) {
      await onDeleteGroup(item.group.id);
    }
  };

  return (
    <aside className="sa-sidebar" aria-label={t("Super-Agent groups", "Super-Agent 群组列表")}>
      <div className="sa-sidebar-head">
        <span>{t("Groups", "协作群组")} ({groups.length})</span>
        <Button size="small" variant="primary" onClick={() => setModalOpen(true)}>
          <Plus size={14} />
          {t("New", "新建")}
        </Button>
      </div>

      <div className="sa-group-list">
        {groups.map((item) => {
          const active = item.group.id === activeGroupId;
          return (
            <div
              key={item.group.id}
              role="button"
              tabIndex={0}
              className={active ? "sa-group-card is-active" : "sa-group-card"}
              onClick={() => onSelectGroup(item.group.id)}
              onKeyDown={(ev) => {
                if (ev.key === "Enter" || ev.key === " ") {
                  ev.preventDefault();
                  onSelectGroup(item.group.id);
                }
              }}
            >
              <div className="sa-group-card-top">
                <span className="sa-group-name">{item.group.name}</span>
                {groups.length > 1 && (
                  <Button
                    size="icon"
                    variant="ghost-danger"
                    aria-label={t("Delete group", "删除群组")}
                    title={t("Delete group", "删除群组")}
                    onClick={(ev) => void handleDelete(ev, item)}
                  >
                    <Trash2 size={14} />
                  </Button>
                )}
              </div>
              <div className="sa-group-meta">
                <span>
                  <i className={`sa-dot ${item.leader_status}`} /> Leader
                </span>
                <span>
                  {t("Runners", "Runner")}: {item.running_worker_count}/{item.worker_count}
                </span>
                {item.open_task_count > 0 && (
                  <span>{t("Tasks", "任务")}: {item.open_task_count}</span>
                )}
                {item.bound_channels.length > 0 && (
                  <span>{item.bound_channels.join(", ")}</span>
                )}
              </div>
              {item.last_message_preview && (
                <div className="sa-group-preview">{item.last_message_preview}</div>
              )}
            </div>
          );
        })}
      </div>

      <Modal
        open={modalOpen}
        title={t("Create Super-Agent group", "新建 Super-Agent 协作群组")}
        description={t(
          "Each group starts with a persistent Leader Agent and supports spawning asynchronous Runners.",
          "每个群组自带常驻 Leader Agent，可随时创建异步 Runner 并接入外部渠道。"
        )}
        size="small"
        onClose={() => setModalOpen(false)}
        footer={
          <>
            <Button variant="secondary" onClick={() => setModalOpen(false)}>
              {t("Cancel", "取消")}
            </Button>
            <Button variant="primary" disabled={creating || !name.trim()} onClick={() => void handleSubmit()}>
              {creating ? t("Creating...", "创建中...") : t("Create group", "创建群组")}
            </Button>
          </>
        }
      >
        <div className="sa-form-grid">
          <label className="sa-field">
            <span>{t("Group name", "群组名称")}</span>
            <input
              className="sa-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("e.g. Backend Engineering Squad", "例如：后端研发群组")}
            />
          </label>
          <label className="sa-field">
            <span>{t("Leader display name", "Leader 名称")}</span>
            <input
              className="sa-input"
              value={leaderName}
              onChange={(e) => setLeaderName(e.target.value)}
            />
          </label>
          <label className="sa-field" style={{ gridColumn: "1 / -1" }}>
            <span>{t("Description", "群组说明")}</span>
            <input
              className="sa-input"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t("Scope, repo goals, or bound channels", "职责边界、协作目标或绑定渠道说明")}
            />
          </label>
        </div>
      </Modal>
    </aside>
  );
}
