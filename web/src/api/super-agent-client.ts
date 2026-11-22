import { apiRequest } from "./api-request";
import type {
  BindChannelPayload,
  ChannelInboundPayload,
  CreateGroupPayload,
  SpawnRunnerPayload,
  SuperAgentConfig,
  SuperAgentOverview,
  SuperChannelBinding,
  SuperGroup,
  SuperGroupDetail,
  SuperGroupMember,
  SuperGroupMessage,
  SuperTask,
  SuperThreadSubscription,
  SuperWatchdogEvent
} from "./super-agent-contracts";

/**
 * 【超级智能体】【Web API 客户端】封装群组对话、Runner 编排、Thread 订阅、多渠道接入与 Watchdog 接口。
 */
export const superAgentApi = {
  /** 读取全局 Super-Agent 概览统计与群组摘要。 */
  overview: () => apiRequest<SuperAgentOverview>("/api/super-agent/overview"),

  /** 创建新的 Super-Agent 群组。 */
  createGroup: (payload: CreateGroupPayload) =>
    apiRequest<{ group: SuperGroup; leader: SuperGroupMember }>("/api/super-agent/groups", {
      method: "POST",
      body: JSON.stringify(payload)
    }),

  /** 读取单个群组的完整详情快照。 */
  getGroup: (groupId: string) =>
    apiRequest<SuperGroupDetail>(`/api/super-agent/groups/${encodeURIComponent(groupId)}`),

  /** 更新群组名称、简介或状态。 */
  updateGroup: (groupId: string, payload: { name?: string; description?: string; status?: string }) =>
    apiRequest<{ group: SuperGroup }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}`, {
      method: "PATCH",
      body: JSON.stringify(payload)
    }),

  /** 删除指定群组。 */
  deleteGroup: (groupId: string) =>
    apiRequest<{ deleted: boolean }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}`, {
      method: "DELETE"
    }),

  /** 在群组内发送消息（支持 @成员 定向交互或由 Leader 自动协调）。 */
  postMessage: (groupId: string, payload: { content: string; sender_name?: string; thread_id?: string; recipient_id?: string; auto_dispatch?: boolean }) =>
    apiRequest<{ message: SuperGroupMessage; route_path: string }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/messages`, {
      method: "POST",
      body: JSON.stringify(payload)
    }),

  /** 创建并异步启动 Worker(Runner)。 */
  spawnRunner: (groupId: string, payload: SpawnRunnerPayload) =>
    apiRequest<{ member: SuperGroupMember; task: SuperTask }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/runners`, {
      method: "POST",
      body: JSON.stringify(payload)
    }),

  /** 向指定 Runner 注入跟进指令。 */
  messageRunner: (groupId: string, runnerId: string, message: string) =>
    apiRequest<{ ok: boolean }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/runners/${encodeURIComponent(runnerId)}/message`, {
      method: "POST",
      body: JSON.stringify({ message })
    }),

  /** 停止指定 Runner。 */
  stopRunner: (groupId: string, runnerId: string, reason?: string) =>
    apiRequest<{ member: SuperGroupMember }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/runners/${encodeURIComponent(runnerId)}/stop`, {
      method: "POST",
      body: JSON.stringify({ reason: reason ?? "" })
    }),

  /** 为 Runner 订阅或取消订阅指定 Thread。 */
  updateSubscription: (groupId: string, runnerId: string, payload: { action: "subscribe" | "unsubscribe"; thread_id: string; mode?: "owner" | "context" }) =>
    apiRequest<{ ok: boolean; subscription?: SuperThreadSubscription; removed?: boolean }>(
      `/api/super-agent/groups/${encodeURIComponent(groupId)}/runners/${encodeURIComponent(runnerId)}/subscribe`,
      {
        method: "POST",
        body: JSON.stringify(payload)
      }
    ),

  /** 重试指定任务。 */
  retryTask: (groupId: string, taskId: string) =>
    apiRequest<{ task: SuperTask }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/tasks/${encodeURIComponent(taskId)}/retry`, {
      method: "POST"
    }),

  /** 停止指定任务及其执行 Runner。 */
  stopTask: (groupId: string, taskId: string) =>
    apiRequest<{ task: SuperTask }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/tasks/${encodeURIComponent(taskId)}/stop`, {
      method: "POST"
    }),

  /** 绑定外部渠道（微信/飞书/QQ/OneBot）到群组。 */
  bindChannel: (groupId: string, payload: BindChannelPayload) =>
    apiRequest<SuperChannelBinding>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/channels`, {
      method: "POST",
      body: JSON.stringify(payload)
    }),

  /** 解除外部渠道绑定。 */
  unbindChannel: (groupId: string, bindingId: string) =>
    apiRequest<{ removed: boolean }>(`/api/super-agent/groups/${encodeURIComponent(groupId)}/channels/${encodeURIComponent(bindingId)}`, {
      method: "DELETE"
    }),

  /** 注入一条渠道消息给群组 Leader 编排并触发成果回推。 */
  sendChannelInbound: (groupId: string, payload: ChannelInboundPayload) =>
    apiRequest<{ group_id: string; message: SuperGroupMessage; route_path: string }>(
      `/api/super-agent/groups/${encodeURIComponent(groupId)}/channel-inbound`,
      {
        method: "POST",
        body: JSON.stringify(payload)
      }
    ),

  /** 立即触发 Watchdog 外部健康巡检。 */
  runWatchdogCheck: () =>
    apiRequest<SuperWatchdogEvent[]>("/api/super-agent/watchdog/check", {
      method: "POST"
    }),

  /** 读取 Super-Agent 运行配置。 */
  getConfig: () => apiRequest<SuperAgentConfig>("/api/super-agent/config"),

  /** 保存 Super-Agent 运行配置。 */
  updateConfig: (config: SuperAgentConfig) =>
    apiRequest<SuperAgentConfig>("/api/super-agent/config", {
      method: "PUT",
      body: JSON.stringify(config)
    })
};
