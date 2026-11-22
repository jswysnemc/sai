import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  Cable,
  ListChecks,
  MessagesSquare,
  Network,
  RefreshCw,
  ShieldCheck
} from "../../shared/ui/icons";
import { api } from "../../api/client";
import type {
  BindChannelPayload,
  ChannelInboundPayload,
  SpawnRunnerPayload,
  SuperAgentConfig
} from "../../api/super-agent-contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { GroupSidebar } from "./group-sidebar";
import { GroupMembersStrip } from "./group-members-strip";
import { GroupChatPanel } from "./group-chat-panel";
import { OrchestrationPanel } from "./orchestration-panel";
import { EventsThreadsPanel } from "./events-threads-panel";
import { ChannelsWatchdogPanel } from "./channels-watchdog-panel";
import { useSuperAgentStream } from "./use-super-agent-stream";
import "./super-agent-page.css";

type WorkspaceTab = "chat" | "tasks" | "threads" | "channels";

/**
 * 【超级智能体】【WebUI 主页面】提供群组对话、Leader 编排、异步 Runner 管理、线程订阅、多渠道接入与 Watchdog 监控。
 *
 * @returns Super-Agent 工作台页面
 */
export function SuperAgentPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { groupId: routeGroupId } = useParams();
  const queryClient = useQueryClient();

  const [tab, setTab] = useState<WorkspaceTab>("chat");
  const [draft, setDraft] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const overview = useQuery({
    queryKey: ["super-agent-overview"],
    queryFn: api.superAgent.overview,
    refetchInterval: 6_000
  });

  const activeGroupId = routeGroupId || overview.data?.groups[0]?.group.id;

  useEffect(() => {
    if (!routeGroupId && overview.data?.groups[0]?.group.id) {
      navigate(`/super-agent/${overview.data.groups[0].group.id}`, { replace: true });
    }
  }, [navigate, overview.data, routeGroupId]);

  useSuperAgentStream(activeGroupId);

  const groupDetail = useQuery({
    queryKey: ["super-agent-group", activeGroupId],
    queryFn: () => api.superAgent.getGroup(activeGroupId!),
    enabled: Boolean(activeGroupId),
    refetchInterval: 5_000
  });

  const configQuery = useQuery({
    queryKey: ["super-agent-config"],
    queryFn: api.superAgent.getConfig
  });

  const invalidateAll = () => {
    void queryClient.invalidateQueries({ queryKey: ["super-agent-overview"] });
    if (activeGroupId) {
      void queryClient.invalidateQueries({ queryKey: ["super-agent-group", activeGroupId] });
    }
    void queryClient.invalidateQueries({ queryKey: ["super-agent-config"] });
  };

  const onError = (error: unknown) => {
    setErrorMessage(error instanceof Error ? error.message : String(error));
  };

  const createGroup = useMutation({
    mutationFn: api.superAgent.createGroup,
    onSuccess: (res) => {
      setErrorMessage(null);
      invalidateAll();
      navigate(`/super-agent/${res.group.id}`);
    },
    onError
  });

  const deleteGroup = useMutation({
    mutationFn: api.superAgent.deleteGroup,
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
      navigate("/super-agent", { replace: true });
    },
    onError
  });

  const sendMessage = useMutation({
    mutationFn: (content: string) => api.superAgent.postMessage(activeGroupId!, { content }),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const sendChannelInbound = useMutation({
    mutationFn: (payload: ChannelInboundPayload) =>
      api.superAgent.sendChannelInbound(activeGroupId!, payload),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const spawnRunner = useMutation({
    mutationFn: (payload: SpawnRunnerPayload) => api.superAgent.spawnRunner(activeGroupId!, payload),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const messageRunner = useMutation({
    mutationFn: ({ runnerId, message }: { runnerId: string; message: string }) =>
      api.superAgent.messageRunner(activeGroupId!, runnerId, message),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const stopRunner = useMutation({
    mutationFn: (runnerId: string) => api.superAgent.stopRunner(activeGroupId!, runnerId),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const updateSubscription = useMutation({
    mutationFn: ({
      runnerId,
      payload
    }: {
      runnerId: string;
      payload: { action: "subscribe" | "unsubscribe"; thread_id: string; mode?: "owner" | "context" };
    }) => api.superAgent.updateSubscription(activeGroupId!, runnerId, payload),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const retryTask = useMutation({
    mutationFn: (taskId: string) => api.superAgent.retryTask(activeGroupId!, taskId),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const stopTask = useMutation({
    mutationFn: (taskId: string) => api.superAgent.stopTask(activeGroupId!, taskId),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const bindChannel = useMutation({
    mutationFn: (payload: BindChannelPayload) => api.superAgent.bindChannel(activeGroupId!, payload),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const unbindChannel = useMutation({
    mutationFn: (bindingId: string) => api.superAgent.unbindChannel(activeGroupId!, bindingId),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const runWatchdog = useMutation({
    mutationFn: api.superAgent.runWatchdogCheck,
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const saveConfig = useMutation({
    mutationFn: (next: SuperAgentConfig) => api.superAgent.updateConfig(next),
    onSuccess: () => {
      setErrorMessage(null);
      invalidateAll();
    },
    onError
  });

  const ov = overview.data;
  const detail = groupDetail.data;
  const busy =
    retryTask.isPending ||
    stopTask.isPending ||
    updateSubscription.isPending ||
    bindChannel.isPending ||
    unbindChannel.isPending ||
    runWatchdog.isPending ||
    saveConfig.isPending;

  return (
    <div className="sa-page">
      <header className="sa-topbar">
        <div className="sa-topbar-inner">
          <div className="sa-topbar-left">
            <Link to="/" className="sa-back-link">
              <ArrowLeft size={16} />
              <span>{t("Workspace", "返回工作台")}</span>
            </Link>
            <div className="sa-title-block">
              <h1>{t("Super-Agent Groups", "Super-Agent 群组协同工作台")}</h1>
              <p>
                {t(
                  "Two-tier Leader / Runner orchestration, multi-member group chat, channel delivery, and Watchdog control.",
                  "双层 Leader / Runner 异步编排、群组成员协同对话、多渠道成果递交与 Watchdog 外部守护。"
                )}
              </p>
            </div>
          </div>
          <div className="sa-topbar-actions">
            <Button size="small" variant="secondary" disabled={runWatchdog.isPending} onClick={() => void runWatchdog.mutateAsync()}>
              <ShieldCheck size={14} />
              {t("Watchdog check", "守护巡检")}
            </Button>
            <Button size="small" variant="secondary" onClick={invalidateAll}>
              <RefreshCw size={14} />
              {t("Refresh", "刷新")}
            </Button>
          </div>
        </div>
      </header>

      {ov && (
        <div className="sa-metrics-bar">
          <span className="sa-metric-pill">
            {t("Groups", "群组")}: <strong>{ov.group_count}</strong>
          </span>
          <span className="sa-metric-pill">
            {t("Running Runners", "运行中 Runner")}: <strong>{ov.running_workers}/{ov.total_workers}</strong>
          </span>
          <span className="sa-metric-pill">
            {t("Active Tasks", "活跃任务")}: <strong>{ov.active_tasks}</strong>
          </span>
          <span className="sa-metric-pill">
            {t("Pending Reports", "待归纳报告")}: <strong>{ov.pending_synthesis_reports}</strong>
          </span>
          <span className="sa-metric-pill">
            {t("Channels", "绑定渠道")}: <strong>{ov.bound_channel_count}</strong>
          </span>
          <span className={ov.watchdog_alerts > 0 ? "sa-metric-pill is-alert" : "sa-metric-pill"}>
            {t("Watchdog Alerts", "守护告警")}: <strong>{ov.watchdog_alerts}</strong>
          </span>
        </div>
      )}

      {errorMessage && <div className="sa-inline-error" style={{ margin: "0.5rem 1rem 0" }}>{errorMessage}</div>}

      <div className="sa-body">
        <GroupSidebar
          groups={ov?.groups ?? []}
          activeGroupId={activeGroupId}
          creating={createGroup.isPending}
          onSelectGroup={(id) => navigate(`/super-agent/${id}`)}
          onCreateGroup={async (payload) => { await createGroup.mutateAsync(payload); }}
          onDeleteGroup={async (id) => { await deleteGroup.mutateAsync(id); }}
        />

        <main className="sa-main">
          {detail ? (
            <>
              <div className="sa-group-header">
                <div className="sa-group-header-info">
                  <h2>{detail.group.name}</h2>
                  <p>{detail.group.description || detail.group.workspace_path}</p>
                </div>
                <nav className="sa-tabs" aria-label={t("Group views", "群组视图切换")}>
                  <button
                    type="button"
                    className={tab === "chat" ? "sa-tab-btn is-active" : "sa-tab-btn"}
                    onClick={() => setTab("chat")}
                  >
                    <MessagesSquare size={14} />
                    {t("Group Chat", "群组对话")}
                  </button>
                  <button
                    type="button"
                    className={tab === "tasks" ? "sa-tab-btn is-active" : "sa-tab-btn"}
                    onClick={() => setTab("tasks")}
                  >
                    <ListChecks size={14} />
                    {t("Tasks & Reports", "任务与报告")} ({detail.tasks.length})
                  </button>
                  <button
                    type="button"
                    className={tab === "threads" ? "sa-tab-btn is-active" : "sa-tab-btn"}
                    onClick={() => setTab("threads")}
                  >
                    <Network size={14} />
                    {t("Threads & Triggers", "订阅与触发器")} ({detail.triggers.length})
                  </button>
                  <button
                    type="button"
                    className={tab === "channels" ? "sa-tab-btn is-active" : "sa-tab-btn"}
                    onClick={() => setTab("channels")}
                  >
                    <Cable size={14} />
                    {t("Channels & Watchdog", "渠道与守护")} ({detail.channels.length})
                  </button>
                </nav>
              </div>

              <GroupMembersStrip
                members={detail.members}
                spawning={spawnRunner.isPending}
                onMentionMember={(name) => {
                  setTab("chat");
                  setDraft((prev) => (prev.startsWith(`@${name} `) ? prev : `@${name} ${prev}`));
                }}
                onSpawnRunner={async (payload) => { await spawnRunner.mutateAsync(payload); }}
                onMessageRunner={async (runnerId, message) => { await messageRunner.mutateAsync({ runnerId, message }); }}
                onStopRunner={async (runnerId) => { await stopRunner.mutateAsync(runnerId); }}
              />

              {tab === "chat" && (
                <GroupChatPanel
                  messages={detail.messages}
                  members={detail.members}
                  draft={draft}
                  sending={sendMessage.isPending || sendChannelInbound.isPending}
                  onDraftChange={setDraft}
                  onSendGroupMessage={async (content) => { await sendMessage.mutateAsync(content); }}
                  onSendChannelInbound={async (payload) => { await sendChannelInbound.mutateAsync(payload); }}
                />
              )}
              {tab === "tasks" && (
                <OrchestrationPanel
                  tasks={detail.tasks}
                  reports={detail.reports}
                  busy={busy}
                  onRetryTask={async (taskId) => { await retryTask.mutateAsync(taskId); }}
                  onStopTask={async (taskId) => { await stopTask.mutateAsync(taskId); }}
                />
              )}
              {tab === "threads" && (
                <EventsThreadsPanel
                  threads={detail.threads}
                  members={detail.members}
                  subscriptions={detail.subscriptions}
                  triggers={detail.triggers}
                  busy={busy}
                  onUpdateSubscription={async (runnerId, payload) => {
                    await updateSubscription.mutateAsync({ runnerId, payload });
                  }}
                />
              )}
              {tab === "channels" && (
                <ChannelsWatchdogPanel
                  channels={detail.channels}
                  deliveries={detail.deliveries}
                  watchdogEvents={ov?.recent_watchdog ?? []}
                  config={configQuery.data}
                  busy={busy}
                  onBindChannel={async (payload) => { await bindChannel.mutateAsync(payload); }}
                  onUnbindChannel={async (bindingId) => { await unbindChannel.mutateAsync(bindingId); }}
                  onRunWatchdog={async () => { await runWatchdog.mutateAsync(); }}
                  onSaveConfig={async (next) => { await saveConfig.mutateAsync(next); }}
                />
              )}
            </>
          ) : (
            <div className="sa-panel-scroll">
              <div className="sa-empty">{t("Loading Super-Agent group...", "正在加载 Super-Agent 群组...")}</div>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
