import { Plus, Trash2, Cable, Webhook } from "lucide-react";
import type { AppConfig, HookItem, McpServerConfig } from "../../api/contracts";

type HooksMcpSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

const HOOK_EVENTS = [
  "agent_start",
  "agent_end",
  "turn_start",
  "turn_end",
  "message_start",
  "message_end",
  "tool_execution_start",
  "tool_execution_end"
];

/**
 * Hooks 与 MCP 配置：对齐 LiveAgent 的精简版。
 */
export function HooksMcpSettingsSection({ config, onConfigChange }: HooksMcpSettingsSectionProps) {
  const hooks = config.hooks ?? { enabled: true, items: [] };
  const mcp = config.mcp ?? { enabled: true, servers: [] };
  const items = hooks.items ?? [];
  const servers = mcp.servers ?? [];

  const setHooks = (patch: Partial<NonNullable<AppConfig["hooks"]>>) => {
    onConfigChange({ ...config, hooks: { ...hooks, ...patch } });
  };
  const setMcp = (patch: Partial<NonNullable<AppConfig["mcp"]>>) => {
    onConfigChange({ ...config, mcp: { ...mcp, ...patch } });
  };

  const updateHook = (index: number, patch: Partial<HookItem>) => {
    const next = items.map((item, i) => (i === index ? { ...item, ...patch } : item));
    setHooks({ items: next });
  };
  const updateServer = (index: number, patch: Partial<McpServerConfig>) => {
    const next = servers.map((item, i) => (i === index ? { ...item, ...patch } : item));
    setMcp({ servers: next });
  };

  return (
    <div className="settings-stack">
      <section className="settings-section-card">
        <header className="settings-section-head">
          <h2><Webhook size={16} /> Hooks</h2>
          <p>在对话生命周期触发 shell 或 HTTP。失败只记日志，不中断主流程。</p>
        </header>
        <label className="settings-check">
          <input
            type="checkbox"
            checked={hooks.enabled !== false}
            onChange={(event) => setHooks({ enabled: event.target.checked })}
          />
          启用 Hooks
        </label>
        <div className="settings-list">
          {items.map((item, index) => (
            <article key={`${item.name}-${index}`} className="settings-item-card">
              <div className="settings-item-row">
                <input
                  value={item.name}
                  onChange={(event) => updateHook(index, { name: event.target.value })}
                  placeholder="名称"
                />
                <select
                  value={item.event}
                  onChange={(event) => updateHook(index, { event: event.target.value })}
                >
                  {HOOK_EVENTS.map((event) => (
                    <option key={event} value={event}>{event}</option>
                  ))}
                </select>
                <select
                  value={item.kind ?? "command"}
                  onChange={(event) => updateHook(index, { kind: event.target.value })}
                >
                  <option value="command">command</option>
                  <option value="http">http</option>
                </select>
                <label className="settings-check">
                  <input
                    type="checkbox"
                    checked={item.enabled !== false}
                    onChange={(event) => updateHook(index, { enabled: event.target.checked })}
                  />
                  启用
                </label>
                <button type="button" className="settings-secondary" onClick={() => setHooks({ items: items.filter((_, i) => i !== index) })}>
                  <Trash2 size={14} />
                </button>
              </div>
              {(item.kind ?? "command") === "command" ? (
                <textarea
                  rows={3}
                  value={item.script ?? ""}
                  onChange={(event) => updateHook(index, { script: event.target.value })}
                  placeholder="shell 脚本，可用 SAI_HOOK_EVENT / SAI_SESSION_ID / SAI_WORKDIR / SAI_TOOL_NAME"
                />
              ) : (
                <textarea
                  rows={3}
                  value={item.requests?.[0]?.url ?? ""}
                  onChange={(event) => updateHook(index, {
                    requests: [{ id: "1", url: event.target.value, method: "POST", body: "" }]
                  })}
                  placeholder="HTTP URL（POST JSON 事件体）"
                />
              )}
            </article>
          ))}
        </div>
        <button
          type="button"
          className="settings-secondary"
          onClick={() => setHooks({
            items: [...items, { name: `hook-${items.length + 1}`, enabled: true, event: "agent_end", kind: "command", script: "echo \"$SAI_HOOK_EVENT $SAI_SESSION_ID\"" }]
          })}
        >
          <Plus size={14} /> 添加 Hook
        </button>
      </section>

      <section className="settings-section-card">
        <header className="settings-section-head">
          <h2><Cable size={16} /> MCP</h2>
          <p>支持 stdio / http / sse。工具注册为 mcp_&lt;server&gt;_&lt;tool&gt;，可用 mcp_manager 查看状态。</p>
        </header>
        <label className="settings-check">
          <input
            type="checkbox"
            checked={mcp.enabled !== false}
            onChange={(event) => setMcp({ enabled: event.target.checked })}
          />
          启用 MCP
        </label>
        <div className="settings-list">
          {servers.map((server, index) => (
            <article key={`${server.id}-${index}`} className="settings-item-card">
              <div className="settings-item-row">
                <input
                  value={server.id}
                  onChange={(event) => updateServer(index, { id: event.target.value })}
                  placeholder="server id"
                />
                <select
                  value={server.transport ?? "stdio"}
                  onChange={(event) => updateServer(index, { transport: event.target.value })}
                >
                  <option value="stdio">stdio</option>
                  <option value="http">http</option>
                  <option value="sse">sse</option>
                </select>
                <label className="settings-check">
                  <input
                    type="checkbox"
                    checked={server.enabled !== false}
                    onChange={(event) => updateServer(index, { enabled: event.target.checked })}
                  />
                  启用
                </label>
                <button type="button" className="settings-secondary" onClick={() => setMcp({ servers: servers.filter((_, i) => i !== index) })}>
                  <Trash2 size={14} />
                </button>
              </div>
              {(server.transport ?? "stdio") === "stdio" ? (
                <>
                  <input
                    value={server.command ?? ""}
                    onChange={(event) => updateServer(index, { command: event.target.value })}
                    placeholder="command，如 npx"
                  />
                  <input
                    value={(server.args ?? []).join(" ")}
                    onChange={(event) => updateServer(index, {
                      args: event.target.value.trim() ? event.target.value.trim().split(/\s+/) : []
                    })}
                    placeholder="args，空格分隔"
                  />
                  <input
                    value={server.cwd ?? ""}
                    onChange={(event) => updateServer(index, { cwd: event.target.value || null })}
                    placeholder="可选 cwd"
                  />
                </>
              ) : (
                <>
                  <input
                    value={server.url ?? ""}
                    onChange={(event) => updateServer(index, { url: event.target.value || null })}
                    placeholder="URL，如 http://127.0.0.1:3000/mcp"
                  />
                  {(server.transport ?? "") === "sse" && (
                    <input
                      value={server.message_url ?? ""}
                      onChange={(event) => updateServer(index, { message_url: event.target.value || null })}
                      placeholder="可选 message_url（缺省从 SSE endpoint 事件解析）"
                    />
                  )}
                </>
              )}
            </article>
          ))}
        </div>
        <button
          type="button"
          className="settings-secondary"
          onClick={() => setMcp({
            servers: [...servers, { id: `server-${servers.length + 1}`, enabled: true, transport: "stdio", command: "", args: [], url: null, message_url: null, headers: {} }]
          })}
        >
          <Plus size={14} /> 添加 MCP Server
        </button>
      </section>
    </div>
  );
}
