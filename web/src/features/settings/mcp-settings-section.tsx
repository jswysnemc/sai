import { Cable, Globe2, Plus, Terminal, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppConfig, McpServerConfig } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import { EditorHeader, SettingsGroup } from "./editor-layout";
import { ObjectListPanel } from "./object-list-panel";
import { KeyValueEditor } from "./key-value-editor";

type McpSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 MCP 服务器列表与详情编辑。
 *
 * @param props 应用配置和更新回调
 * @returns MCP 配置区域
 */
export function McpSettingsSection({ config, onConfigChange }: McpSettingsSectionProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const mcp = config.mcp ?? { enabled: true, servers: [] };
  const servers = mcp.servers ?? [];
  const [selectedId, setSelectedId] = useState(servers[0]?.id ?? "");

  useEffect(() => {
    if (!servers.some((server) => server.id === selectedId)) {
      setSelectedId(servers[0]?.id ?? "");
    }
  }, [servers, selectedId]);

  const selectedIndex = Math.max(0, servers.findIndex((server) => server.id === selectedId));
  const server = servers[selectedIndex];

  const setMcp = (patch: Partial<NonNullable<AppConfig["mcp"]>>) => {
    onConfigChange({ ...config, mcp: { ...mcp, ...patch } });
  };

  const updateServer = (index: number, patch: Partial<McpServerConfig>) => {
    const next = servers.map((item, i) => (i === index ? { ...item, ...patch } : item));
    setMcp({ servers: next });
    if (index === selectedIndex && patch.id !== undefined) setSelectedId(patch.id);
  };

  const addServer = () => {
    const id = uniqueServerId(servers);
    const next: McpServerConfig = {
      id,
      enabled: true,
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-filesystem", "."],
      env: {},
      cwd: null,
      url: null,
      message_url: null,
      headers: {},
      timeout_ms: 30_000
    };
    setMcp({ servers: [...servers, next] });
    setSelectedId(id);
  };

  const deleteServer = async () => {
    if (!server) return;
    const confirmed = await confirm({
      title: t("Delete MCP server", "删除 MCP 服务"),
      description: t(`Delete “${server.id}” and stop exposing its tools.`, `删除“${server.id}”，其工具将不再暴露。`),
      confirmLabel: t("Delete server", "删除服务"),
      danger: true
    });
    if (!confirmed) return;
    const next = servers.filter((_, index) => index !== selectedIndex);
    setMcp({ servers: next });
    setSelectedId(next[0]?.id ?? "");
  };

  if (!server) {
    return (
      <div className="settings-objects-layout">
        <ObjectListPanel
          title="MCP"
          items={[]}
          selectedId=""
          searchPlaceholder={t("Search MCP servers", "搜索 MCP 服务")}
          addLabel={t("Add MCP server", "添加 MCP 服务")}
          onSelect={() => undefined}
          onAdd={addServer}
          headerSlot={
            <label className="settings-toggle-field object-list-toggle">
              <span>
                <strong>{t("Enable MCP", "启用 MCP")}</strong>
                <small>{t("Master switch for external tool servers", "外部工具服务总开关")}</small>
              </span>
              <input
                type="checkbox"
                checked={mcp.enabled !== false}
                onChange={(event) => setMcp({ enabled: event.target.checked })}
              />
            </label>
          }
        />
        <section className="settings-editor">
          <div className="settings-empty">
            <p>{t("No MCP servers yet. Connect stdio, HTTP, or SSE servers to expose tools as mcp_<server>_<tool>.", "还没有 MCP 服务。可接入 stdio / HTTP / SSE，工具会注册为 mcp_<server>_<tool>。")}</p>
            <button type="button" className="settings-secondary" onClick={addServer}>
              <Plus size={14} />{t("Add MCP server", "添加 MCP 服务")}
            </button>
          </div>
        </section>
      </div>
    );
  }

  const transport = server.transport ?? "stdio";
  const transportOptions = [
    { value: "stdio", label: t("stdio (local process)", "stdio（本地进程）") },
    { value: "http", label: t("HTTP", "HTTP") },
    { value: "sse", label: t("SSE", "SSE") }
  ];

  return (
    <div className="settings-objects-layout">
      <ObjectListPanel
        title="MCP"
        items={servers.map((item) => ({
          id: item.id,
          name: item.id,
          meta: transportMeta(item.transport ?? "stdio", item, t),
          icon: (item.transport ?? "stdio") === "stdio" ? <Terminal size={14} /> : <Globe2 size={14} />,
          marked: item.enabled !== false
        }))}
        selectedId={selectedId}
        searchPlaceholder={t("Search MCP servers", "搜索 MCP 服务")}
        addLabel={t("Add MCP server", "添加 MCP 服务")}
        onSelect={setSelectedId}
        onAdd={addServer}
        headerSlot={
          <label className="settings-toggle-field object-list-toggle">
            <span>
              <strong>{t("Enable MCP", "启用 MCP")}</strong>
              <small>{t("Master switch for external tool servers", "外部工具服务总开关")}</small>
            </span>
            <input
              type="checkbox"
              checked={mcp.enabled !== false}
              onChange={(event) => setMcp({ enabled: event.target.checked })}
            />
          </label>
        }
      />
      <section className="settings-editor">
        <EditorHeader
          kicker="MCP"
          title={server.id}
          description={t(
            "Tools register as mcp_<server>_<tool>. Use the mcp_manager tool in chat to inspect live status.",
            "工具注册为 mcp_<server>_<tool>。可在对话中用 mcp_manager 查看运行状态。"
          )}
          actions={
            <>
              <label className="settings-switch">
                <input
                  type="checkbox"
                  checked={server.enabled !== false}
                  onChange={(event) => updateServer(selectedIndex, { enabled: event.target.checked })}
                />
                <span />
                <strong>{server.enabled !== false ? t("Enabled", "已启用") : t("Disabled", "已禁用")}</strong>
              </label>
              <button type="button" className="settings-danger" onClick={() => void deleteServer()}>
                <Trash2 size={14} />{t("Delete", "删除")}
              </button>
            </>
          }
        />

        <SettingsGroup
          title={t("Server identity", "服务标识")}
          description={t("Stable id and transport used to reach the server", "稳定标识与连接方式")}
        >
          <div className="settings-form-grid">
            <label className="settings-field">
              <span>{t("Server ID", "服务 ID")}</span>
              <input
                value={server.id}
                onChange={(event) => updateServer(selectedIndex, { id: event.target.value.trim() || server.id })}
                spellCheck={false}
              />
              <small>{t("Used in tool names: mcp_<id>_<tool>", "会出现在工具名：mcp_<id>_<tool>")}</small>
            </label>
            <div className="settings-field">
              <span>{t("Transport", "传输方式")}</span>
              <Select
                value={transport}
                options={transportOptions}
                onChange={(value) => updateServer(selectedIndex, { transport: value })}
                ariaLabel={t("MCP transport", "MCP 传输方式")}
              />
              <small>{t("stdio for local CLIs; HTTP/SSE for remote endpoints", "本地 CLI 用 stdio，远程端点用 HTTP/SSE")}</small>
            </div>
            <label className="settings-field">
              <span>{t("Timeout (ms)", "超时（毫秒）")}</span>
              <input
                type="number"
                min={100}
                max={300000}
                value={server.timeout_ms ?? 30_000}
                onChange={(event) => updateServer(selectedIndex, {
                  timeout_ms: event.target.value === "" ? null : Number(event.target.value)
                })}
              />
              <small>{t("Request / process startup timeout", "请求或进程启动超时")}</small>
            </label>
          </div>
        </SettingsGroup>

        {transport === "stdio" ? (
          <SettingsGroup
            title={t("stdio process", "stdio 进程")}
            description={t("Local command that speaks MCP over stdin/stdout", "通过 stdin/stdout 对话的本地命令")}
          >
            <div className="settings-form-grid">
              <label className="settings-field">
                <span>{t("Command", "命令")}</span>
                <input
                  value={server.command ?? ""}
                  onChange={(event) => updateServer(selectedIndex, { command: event.target.value })}
                  spellCheck={false}
                  placeholder="npx"
                />
                <small>{t("Executable on PATH, e.g. npx / uvx / node", "PATH 上的可执行文件，如 npx / uvx / node")}</small>
              </label>
              <label className="settings-field">
                <span>{t("Working directory", "工作目录")}</span>
                <input
                  value={server.cwd ?? ""}
                  onChange={(event) => updateServer(selectedIndex, { cwd: event.target.value || null })}
                  spellCheck={false}
                  placeholder={t("Optional; defaults to workspace", "可选；默认工作区")}
                />
              </label>
              <label className="settings-field full">
                <span>{t("Arguments", "参数")}</span>
                <textarea
                  rows={3}
                  value={(server.args ?? []).join("\n")}
                  onChange={(event) => updateServer(selectedIndex, {
                    args: event.target.value
                      .split(/\r?\n/)
                      .map((line) => line.trim())
                      .filter(Boolean)
                  })}
                  spellCheck={false}
                  placeholder={"-y\n@modelcontextprotocol/server-filesystem\n."}
                />
                <small>{t("One argument per line (safer than space splitting)", "每行一个参数，比空格拆分更稳")}</small>
              </label>
              <div className="settings-field full">
                <span>{t("Environment", "环境变量")}</span>
                <KeyValueEditor
                  value={server.env ?? {}}
                  keyPlaceholder={t("Variable name", "变量名")}
                  valuePlaceholder={t("Value", "值")}
                  addLabel={t("Add environment variable", "添加环境变量")}
                  onChange={(env) => updateServer(selectedIndex, { env })}
                />
                <small>{t("Extra env for the child process", "子进程额外环境变量")}</small>
              </div>
            </div>
          </SettingsGroup>
        ) : (
          <SettingsGroup
            title={t("Remote endpoint", "远程端点")}
            description={transport === "sse"
              ? t("SSE stream plus optional dedicated message URL", "SSE 流，以及可选独立 message URL")
              : t("Streamable HTTP MCP endpoint", "Streamable HTTP MCP 端点")}
          >
            <div className="settings-form-grid">
              <label className="settings-field full">
                <span>URL</span>
                <input
                  value={server.url ?? ""}
                  onChange={(event) => updateServer(selectedIndex, { url: event.target.value || null })}
                  spellCheck={false}
                  placeholder="http://127.0.0.1:3000/mcp"
                />
                <small>{t("Base MCP endpoint URL", "MCP 基础端点")}</small>
              </label>
              {transport === "sse" && (
                <label className="settings-field full">
                  <span>message_url</span>
                  <input
                    value={server.message_url ?? ""}
                    onChange={(event) => updateServer(selectedIndex, { message_url: event.target.value || null })}
                    spellCheck={false}
                    placeholder={t("Optional; parsed from SSE endpoint event when empty", "可选；留空时从 SSE endpoint 事件解析")}
                  />
                </label>
              )}
              <div className="settings-field full">
                <span>{t("Headers", "请求头")}</span>
                <KeyValueEditor
                  value={server.headers ?? {}}
                  keyPlaceholder={t("Header name", "Header 名")}
                  valuePlaceholder={t("Header value", "Header 值")}
                  addLabel={t("Add header", "添加请求头")}
                  onChange={(headers) => updateServer(selectedIndex, { headers })}
                />
                <small>{t("Auth headers or custom routing metadata", "鉴权头或自定义路由元数据")}</small>
              </div>
            </div>
          </SettingsGroup>
        )}

        <div className="settings-note-card">
          <Cable size={15} />
          <div>
            <strong>{t("Tool naming", "工具命名")}</strong>
            <p>
              {t(
                `Tools from this server appear as mcp_${server.id || "<id>"}_<tool_name>. Keep the server id short and stable.`,
                `该服务工具会显示为 mcp_${server.id || "<id>"}_<tool_name>。服务 ID 建议简短且稳定。`
              )}
            </p>
          </div>
        </div>
      </section>
    </div>
  );
}

function uniqueServerId(servers: McpServerConfig[]): string {
  let suffix = servers.length + 1;
  let id = `server-${suffix}`;
  while (servers.some((server) => server.id === id)) {
    suffix += 1;
    id = `server-${suffix}`;
  }
  return id;
}

function transportMeta(
  transport: string,
  server: McpServerConfig,
  t: (en: string, zh: string) => string
): string {
  if (transport === "stdio") {
    const command = [server.command, ...(server.args ?? []).slice(0, 1)].filter(Boolean).join(" ");
    return command || t("stdio", "stdio");
  }
  return server.url || transport;
}
