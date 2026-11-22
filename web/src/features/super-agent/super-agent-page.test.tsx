import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type {
  SuperAgentConfig,
  SuperAgentOverview,
  SuperGroupDetail
} from "../../api/super-agent-contracts";
import { DialogProvider } from "../../shared/ui/dialog/dialog-provider";
import { renderWithProviders } from "../../shared/testing/render-with-providers";
import { I18nProvider } from "../i18n/i18n-context";
import { ChannelsWatchdogPanel } from "./channels-watchdog-panel";
import { EventsThreadsPanel } from "./events-threads-panel";
import { OrchestrationPanel } from "./orchestration-panel";
import { SuperAgentPage } from "./super-agent-page";

const now = Date.now();

const sampleOverview: SuperAgentOverview = {
  group_count: 1,
  running_workers: 1,
  total_workers: 1,
  active_tasks: 1,
  pending_synthesis_reports: 0,
  delivered_tasks_24h: 1,
  triggers_24h: 3,
  fast_path_triggers_24h: 1,
  slow_path_triggers_24h: 2,
  watchdog_alerts: 0,
  bound_channel_count: 1,
  groups: [
    {
      group: {
        id: "grp_1",
        name: "Core Platform Squad",
        description: "Leader + Runners",
        workspace_path: "/repo",
        status: "active",
        leader_member_id: "mem_leader",
        default_thread_id: "thr_1",
        created_at: now,
        updated_at: now
      },
      leader_status: "idle",
      worker_count: 1,
      running_worker_count: 1,
      open_task_count: 1,
      pending_review_count: 0,
      message_count: 2,
      bound_channels: ["weixin"],
      last_message_preview: "已完成后端接口联调",
      last_message_at: now
    }
  ],
  recent_watchdog: [
    {
      id: "wd_1",
      group_id: null,
      severity: "healthy",
      kind: "watchdog_check",
      summary: "All runners healthy",
      details: "",
      resolved: true,
      created_at: now
    }
  ]
};

const sampleDetail: SuperGroupDetail = {
  group: sampleOverview.groups[0].group,
  members: [
    {
      id: "mem_leader",
      group_id: "grp_1",
      name: "Leader",
      role: "leader",
      status: "idle",
      specialty: "orchestrator",
      subagent_type: "leader",
      model_label: "deepseek/chat",
      workdir: "/repo",
      last_activity: "ready",
      last_heartbeat_at: now,
      created_at: now,
      updated_at: now
    },
    {
      id: "mem_runner",
      group_id: "grp_1",
      name: "backend-runner",
      role: "worker",
      status: "running",
      specialty: "rust",
      subagent_type: "general",
      model_label: "deepseek/reasoner",
      workdir: "/repo",
      last_activity: "running cargo test",
      last_heartbeat_at: now,
      created_at: now,
      updated_at: now
    }
  ],
  threads: [
    {
      id: "thr_1",
      group_id: "grp_1",
      title: "Main Thread",
      channel: "group",
      status: "open",
      created_at: now,
      updated_at: now
    }
  ],
  subscriptions: [
    {
      id: "sub_1",
      group_id: "grp_1",
      thread_id: "thr_1",
      member_id: "mem_runner",
      mode: "owner",
      created_at: now
    }
  ],
  messages: [
    {
      seq: 1,
      id: "msg_1",
      group_id: "grp_1",
      thread_id: "thr_1",
      sender_kind: "worker",
      sender_id: "mem_runner",
      sender_name: "backend-runner",
      recipient_id: "mem_leader",
      recipient_name: "Leader",
      kind: "report",
      content: "单元测试全部通过",
      created_at: now
    },
    {
      seq: 2,
      id: "msg_2",
      group_id: "grp_1",
      thread_id: "thr_1",
      sender_kind: "leader",
      sender_id: "mem_leader",
      sender_name: "Leader",
      recipient_id: "user",
      recipient_name: "User",
      kind: "delivery",
      channel_source: "weixin",
      content: "已完成后端接口改造并回推微信。",
      created_at: now
    }
  ],
  tasks: [
    {
      id: "tsk_1",
      group_id: "grp_1",
      thread_id: "thr_1",
      title: "Refactor gateway bridge",
      objective: "Wire multi-channel routing",
      assigned_member_id: "mem_runner",
      assigned_member_name: "backend-runner",
      origin_channel: "weixin",
      origin_target: {},
      status: "delivered",
      deliverable: "全部测试通过",
      created_at: now,
      updated_at: now
    }
  ],
  reports: [
    {
      id: "rep_1",
      group_id: "grp_1",
      thread_id: "thr_1",
      task_id: "tsk_1",
      worker_member_id: "mem_runner",
      worker_name: "backend-runner",
      status: "ready",
      summary: "所有单元测试已通过",
      details: "cargo test green",
      artifacts: ["src/super_agent/gateway_bridge.rs"],
      synthesized_by_leader: true,
      created_at: now
    }
  ],
  triggers: [
    {
      id: "trg_1",
      group_id: "grp_1",
      thread_id: "thr_1",
      source_channel: "weixin",
      actor: "Alice",
      content: "请修复网关路由",
      disposition: "evaluated",
      route_path: "slow_path",
      reason: "leader_orchestration",
      target_member_ids: ["mem_leader"],
      metadata: {},
      created_at: now,
      updated_at: now
    }
  ],
  channels: [
    {
      id: "ch_1",
      group_id: "grp_1",
      channel: "weixin",
      external_chat_id: "wx_owner",
      label: "Owner Weixin",
      admission_mode: "auto",
      enabled: true,
      created_at: now
    }
  ],
  deliveries: [
    {
      id: "dlv_1",
      group_id: "grp_1",
      task_id: "tsk_1",
      channel: "weixin",
      target: "weixin:wx_owner",
      status: "sent",
      preview: "已完成后端接口改造",
      created_at: now
    }
  ]
};

const sampleConfig: SuperAgentConfig = {
  enabled: true,
  channel_leader_routing: true,
  leader_provider_id: "",
  leader_model: "",
  worker_provider_id: "",
  worker_model: "",
  max_concurrent_workers: 4,
  leader_batch_limit: 8,
  im_reply_max_words: 180,
  watchdog_enabled: true,
  watchdog_interval_seconds: 60,
  worker_stall_timeout_seconds: 300,
  worker_use_worktree: true
};

describe("SuperAgentPage and Panels", () => {
  it("renders group workspace with Leader, Runner, reports, and Leader delivery", () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false, gcTime: 0 } }
    });
    client.setQueryData(["super-agent-overview"], sampleOverview);
    client.setQueryData(["super-agent-group", "grp_1"], sampleDetail);
    client.setQueryData(["super-agent-config"], sampleConfig);

    const html = renderToStaticMarkup(
      <QueryClientProvider client={client}>
        <I18nProvider>
          <DialogProvider>
            <MemoryRouter initialEntries={["/super-agent/grp_1"]}>
              <Routes>
                <Route path="/super-agent/:groupId" element={<SuperAgentPage />} />
              </Routes>
            </MemoryRouter>
          </DialogProvider>
        </I18nProvider>
      </QueryClientProvider>
    );

    expect(html).toContain("Core Platform Squad");
    expect(html).toContain("backend-runner");
    expect(html).toContain("已完成后端接口改造并回推微信。");
    expect(html).toContain("单元测试全部通过");
    expect(html).toContain("via weixin");
  });

  it("renders tasks, reports, thread subscriptions, triggers, channels, and watchdog panels", () => {
    const orchHtml = renderWithProviders(
      <OrchestrationPanel
        tasks={sampleDetail.tasks}
        reports={sampleDetail.reports}
        busy={false}
        onRetryTask={async () => {}}
        onStopTask={async () => {}}
      />
    );
    expect(orchHtml).toContain("Refactor gateway bridge");
    expect(orchHtml).toContain("所有单元测试已通过");
    expect(orchHtml).toContain("src/super_agent/gateway_bridge.rs");

    const threadsHtml = renderWithProviders(
      <EventsThreadsPanel
        threads={sampleDetail.threads}
        members={sampleDetail.members}
        subscriptions={sampleDetail.subscriptions}
        triggers={sampleDetail.triggers}
        busy={false}
        onUpdateSubscription={async () => {}}
      />
    );
    expect(threadsHtml).toContain("@backend-runner");
    expect(threadsHtml).toContain("slow_path");
    expect(threadsHtml).toContain("请修复网关路由");

    const channelsHtml = renderWithProviders(
      <ChannelsWatchdogPanel
        channels={sampleDetail.channels}
        deliveries={sampleDetail.deliveries}
        watchdogEvents={sampleOverview.recent_watchdog}
        config={sampleConfig}
        busy={false}
        onBindChannel={async () => {}}
        onUnbindChannel={async () => {}}
        onRunWatchdog={async () => {}}
        onSaveConfig={async () => {}}
      />
    );
    expect(channelsHtml).toContain("Owner Weixin");
    expect(channelsHtml).toContain("weixin:wx_owner");
    expect(channelsHtml).toContain("All runners healthy");
  });
});
