import { useState } from "react";
import { Network, Route, Trash2 } from "../../shared/ui/icons";
import type {
  SuperGroupMember,
  SuperGroupThread,
  SuperThreadSubscription,
  SuperTriggerEvent
} from "../../api/super-agent-contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";

type EventsThreadsPanelProps = {
  threads: SuperGroupThread[];
  members: SuperGroupMember[];
  subscriptions: SuperThreadSubscription[];
  triggers: SuperTriggerEvent[];
  busy: boolean;
  onUpdateSubscription: (
    runnerId: string,
    payload: { action: "subscribe" | "unsubscribe"; thread_id: string; mode?: "owner" | "context" }
  ) => Promise<void>;
};

/**
 * 【超级智能体】【线程订阅与触发器面板】管理 Thread-to-Worker 多对多订阅直通关系，并审计两级 Fast/Slow Path Trigger 路由。
 *
 * @param props 线程、成员、订阅与触发器数据
 * @returns 线程与触发器审计面板
 */
export function EventsThreadsPanel({
  threads,
  members,
  subscriptions,
  triggers,
  busy,
  onUpdateSubscription
}: EventsThreadsPanelProps) {
  const { t } = useI18n();
  const workers = members.filter((m) => m.role === "worker");
  const [selectedThread, setSelectedThread] = useState(threads[0]?.id ?? "");
  const [selectedWorker, setSelectedWorker] = useState(workers[0]?.id ?? "");
  const [subMode, setSubMode] = useState<"owner" | "context">("owner");

  const effectiveThread = selectedThread || threads[0]?.id || "";
  const effectiveWorker = selectedWorker || workers[0]?.id || "";

  const memberName = (id: string) => members.find((m) => m.id === id)?.name ?? id;
  const threadTitle = (id: string) => threads.find((th) => th.id === id)?.title ?? id;

  return (
    <div className="sa-panel-scroll">
      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <Network size={16} />
            {t("Threads & Fast-Path Subscriptions", "线程与多对多订阅直通路由")} ({subscriptions.length})
          </span>
        </header>

        <div className="sa-form-grid">
          <label className="sa-field">
            <span>{t("Thread", "目标线程")}</span>
            <select className="sa-select" value={effectiveThread} onChange={(e) => setSelectedThread(e.target.value)}>
              {threads.map((th) => (
                <option key={th.id} value={th.id}>
                  {th.title} ({th.channel})
                </option>
              ))}
            </select>
          </label>
          <label className="sa-field">
            <span>{t("Runner", "订阅 Runner")}</span>
            <select className="sa-select" value={effectiveWorker} onChange={(e) => setSelectedWorker(e.target.value)}>
              {workers.length === 0 ? (
                <option value="">{t("No runners yet", "暂无 Runner")}</option>
              ) : (
                workers.map((w) => (
                  <option key={w.id} value={w.id}>
                    {w.name} ({w.status})
                  </option>
                ))
              )}
            </select>
          </label>
          <label className="sa-field">
            <span>{t("Mode", "订阅模式")}</span>
            <select className="sa-select" value={subMode} onChange={(e) => setSubMode(e.target.value as "owner" | "context")}>
              <option value="owner">owner (Fast-Path Fan-Out)</option>
              <option value="context">context (Read Only)</option>
            </select>
          </label>
          <Button
            variant="primary"
            size="small"
            disabled={busy || !effectiveThread || !effectiveWorker}
            onClick={() =>
              void onUpdateSubscription(effectiveWorker, {
                action: "subscribe",
                thread_id: effectiveThread,
                mode: subMode
              })
            }
          >
            {t("Bind subscription", "绑定订阅")}
          </Button>
        </div>

        {subscriptions.length === 0 ? (
          <div className="sa-empty">
            {t("No thread subscriptions yet. Spawning a Runner automatically subscribes it as owner.", "暂无订阅记录。创建 Runner 时会自动将其绑定为对应线程的 owner。")}
          </div>
        ) : (
          <div className="sa-grid-2">
            {subscriptions.map((sub) => (
              <div key={sub.id} className="sa-item-card">
                <div className="sa-group-card-top">
                  <strong>@{memberName(sub.member_id)}</strong>
                  <div style={{ display: "flex", alignItems: "center", gap: "0.3rem" }}>
                    <span className="sa-badge accent">{sub.mode}</span>
                    <Button
                      size="icon"
                      variant="ghost-danger"
                      aria-label={t("Remove subscription", "取消订阅")}
                      disabled={busy}
                      onClick={() =>
                        void onUpdateSubscription(sub.member_id, {
                          action: "unsubscribe",
                          thread_id: sub.thread_id
                        })
                      }
                    >
                      <Trash2 size={14} />
                    </Button>
                  </div>
                </div>
                <div className="sa-group-meta">
                  {t("Thread", "线程")}: {threadTitle(sub.thread_id)}
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <Route size={16} />
            {t("Two-Stage Trigger Routing Log", "两级触发器路由审计（Fast Path / Slow Path）")} ({triggers.length})
          </span>
        </header>

        {triggers.length === 0 ? (
          <div className="sa-empty">{t("No triggers recorded in this group yet.", "本群组暂无触发器记录。")}</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.45rem" }}>
            {triggers.slice(0, 30).map((tr) => (
              <article key={tr.id} className="sa-item-card">
                <div className="sa-group-card-top">
                  <div style={{ display: "flex", flexWrap: "wrap", gap: "0.35rem", alignItems: "center" }}>
                    <span className={tr.route_path === "fast_path" ? "sa-badge accent" : "sa-badge"}>
                      {tr.route_path}
                    </span>
                    <span className="sa-badge">{tr.disposition}</span>
                    <span className="sa-badge">{tr.source_channel}</span>
                    <strong style={{ fontSize: "0.8rem" }}>{tr.actor}</strong>
                  </div>
                  <span style={{ fontSize: "0.72rem", color: "var(--ink-soft)" }}>
                    {new Date(tr.created_at).toLocaleTimeString()}
                  </span>
                </div>
                <div style={{ fontSize: "0.8rem" }}>{tr.content}</div>
                <div className="sa-group-meta">{t("Reason", "路由原因")}: {tr.reason}</div>
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
