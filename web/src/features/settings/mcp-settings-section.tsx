import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Braces, Cable, FormInput, Globe2, Plus, Save, Terminal, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { McpConfig, McpServerConfig } from "../../api/contracts";
import { toDisplayError } from "../../api/api-error";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { JsonCodeEditor } from "../../shared/ui/code-editor/json-code-editor";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import { EditorHeader, SettingsGroup } from "./editor-layout";
import { ObjectListPanel } from "./object-list-panel";
import { KeyValueEditor } from "./key-value-editor";
import { McpToolBrowser } from "./mcp-tool-browser";

type EditorMode = "form" | "json";

/**
 * 渲染独立 MCP 配置（`~/.config/sai/mcp.jsonc`）。
 *
 * 支持结构化表单和完整 JSON 两种编辑方式，保存都写入独立配置文件。
 *
 * @returns MCP 配置区域
 */
export function McpSettingsSection() {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const response = useQuery({ queryKey: ["mcp-config"], queryFn: api.config.loadMcp });
  const [mcp, setMcp] = useState<McpConfig | null>(null);
  const [raw, setRaw] = useState("");
  const [dirty, setDirty] = useState(false);
  const [mode, setMode] = useState<EditorMode>("form");
  const [selectedId, setSelectedId] = useState("");
  const [parseError, setParseError] = useState<string | null>(null);
  const [scannedServerId, setScannedServerId] = useState("");

  useEffect(() => {
    if (!response.data || dirty) return;
    setMcp(response.data.config);
    setRaw(JSON.stringify(response.data.config, null, 2));
    setParseError(null);
  }, [response.data, dirty]);

  const servers = mcp?.servers ?? [];
  useEffect(() => {
    if (!servers.some((server) => server.id === selectedId)) {
      setSelectedId(servers[0]?.id ?? "");
    }
  }, [servers, selectedId]);

  const selectedIndex = Math.max(0, servers.findIndex((server) => server.id === selectedId));
  const server = servers[selectedIndex];

  const save = useMutation({
    mutationFn: async () => {
      const config = mode === "json" ? parseMcpJson(raw) : mcp;
      if (!config) throw new Error(t("MCP config is not loaded", "MCP 配置尚未加载"));
      return api.config.saveMcp(config);
    },
    onSuccess: (saved) => {
      setMcp(saved.config);
      setRaw(JSON.stringify(saved.config, null, 2));
      setDirty(false);
      setParseError(null);
      queryClient.setQueryData(["mcp-config"], saved);
    }
  });

  const scanTools = useMutation({
    mutationFn: (target: McpServerConfig) => api.config.scanMcpTools(target),
    onSuccess: (_, target) => setScannedServerId(target.id)
  });

  const markDirty = () => {
    setDirty(true);
    save.reset();
  };

  const updateMcp = (next: McpConfig) => {
    setMcp(next);
    setRaw(JSON.stringify(next, null, 2));
    setParseError(null);
    markDirty();
  };

  const patchMcp = (patch: Partial<McpConfig>) => {
    if (!mcp) return;
    updateMcp({ ...mcp, ...patch });
  };

  const updateServer = (index: number, patch: Partial<McpServerConfig>) => {
    if (!mcp) return;
    const nextServers = servers.map((item, i) => (i === index ? { ...item, ...patch } : item));
    updateMcp({ ...mcp, servers: nextServers });
    scanTools.reset();
    setScannedServerId("");
    if (index === selectedIndex && patch.id !== undefined) setSelectedId(patch.id);
  };

  const addServer = () => {
    if (!mcp) return;
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
    updateMcp({ ...mcp, servers: [...servers, next] });
    setSelectedId(id);
  };

  const deleteServer = async () => {
    if (!mcp || !server) return;
    const confirmed = await confirm({
      title: t("Delete MCP server", "删除 MCP 服务"),
      description: t(`Delete “${server.id}” and stop exposing its tools.`, `删除“${server.id}”，其工具将不再暴露。`),
      confirmLabel: t("Delete server", "删除服务"),
      danger: true
    });
    if (!confirmed) return;
    const next = servers.filter((_, index) => index !== selectedIndex);
    updateMcp({ ...mcp, servers: next });
    setSelectedId(next[0]?.id ?? "");
  };

  /** 在表单与 JSON 模式间切换，尽量保留未保存改动。 */
  const switchMode = (next: EditorMode) => {
    if (next === mode) return;
    if (next === "json") {
      if (mcp) setRaw(JSON.stringify(mcp, null, 2));
      setParseError(null);
      setMode("json");
      return;
    }
    try {
      const parsed = parseMcpJson(raw);
      setMcp(parsed);
      setParseError(null);
      setMode("form");
    } catch (error) {
      setParseError(error instanceof Error ? error.message : String(error));
    }
  };

  const updateRaw = (value: string) => {
    setRaw(value);
    setParseError(null);
    markDirty();
    // 1. JSON 合法时同步到表单状态，便于切回表单不丢内容
    try {
      setMcp(parseMcpJson(value));
    } catch {
      // 输入中途不合法时保留上一份 mcp，切模式时再强制校验
    }
  };

  if (response.isLoading || !mcp) {
    return <div className="settings-state">{t("Loading MCP configuration", "正在读取 MCP 配置")}</div>;
  }

  const error = (response.error ?? save.error)
    ? toDisplayError(response.error ?? save.error, "MCP configuration error", "MCP 配置错误")
    : null;
  const path = response.data?.path ?? "~/.config/sai/mcp.jsonc";
  const transport = server?.transport ?? "stdio";
  const transportOptions = [
    { value: "stdio", label: t("stdio (local process)", "stdio（本地进程）") },
    { value: "http", label: t("HTTP", "HTTP") },
    { value: "sse", label: t("SSE", "SSE") }
  ];

  const saveBar = (
    <Button
      className="settings-secondary"
      disabled={!dirty || save.isPending}
      onClick={() => void save.mutateAsync().catch((cause) => {
        if (mode === "json") {
          setParseError(cause instanceof Error ? cause.message : String(cause));
        }
      })}
    >
      <Save size={14} />{save.isPending ? t("Saving", "正在保存") : dirty ? t("Save MCP", "保存 MCP") : t("Saved", "已保存")}
    </Button>
  );

  return (
    <div className={mode === "json" ? "settings-editor advanced-settings mcp-json-layout" : "settings-objects-layout"}>
      {mode === "form" && (
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
                <small>{t("Stored in a separate mcp.jsonc file", "保存在独立的 mcp.jsonc")}</small>
              </span>
              <input
                type="checkbox"
                checked={mcp.enabled !== false}
                onChange={(event) => patchMcp({ enabled: event.target.checked })}
              />
            </label>
          }
        />
      )}

      <section className="settings-editor">
        <EditorHeader
          kicker="MCP"
          title={mode === "json" ? t("MCP JSON", "MCP JSON") : (server?.id || t("MCP servers", "MCP 服务"))}
          description={t(
            `File: ${path}. Form and JSON edit the same independent config.`,
            `文件：${path}。表单与 JSON 编辑同一份独立配置。`
          )}
          actions={
            <>
              <nav className="settings-tabs mcp-mode-tabs" aria-label={t("MCP editor mode", "MCP 编辑模式")}>
                <Button className={mode === "form" ? "settings-secondary active" : "settings-secondary"} onClick={() => switchMode("form")}>
                  <FormInput size={13} />{t("Form", "表单")}
                </Button>
                <Button className={mode === "json" ? "settings-secondary active" : "settings-secondary"} onClick={() => switchMode("json")}>
                  <Braces size={13} />JSON
                </Button>
              </nav>
              {saveBar}
              {mode === "form" && server && (
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
                  <Button variant="danger" onClick={() => void deleteServer()}>
                    <Trash2 size={14} />{t("Delete", "删除")}
                  </Button>
                </>
              )}
            </>
          }
        />

        {(error || parseError) && (
          <div className="settings-inline-error">{parseError ?? error?.message}</div>
        )}

        {mode === "json" ? (
          <>
            <div className="advanced-settings-note">
              {t(
                "Edit the full mcp.jsonc content. Saving validates transport, server ids, and timeouts on the server.",
                "编辑完整 mcp.jsonc。保存时服务端会校验传输方式、服务 ID 和超时。"
              )}
            </div>
            <JsonCodeEditor
              value={raw}
              onChange={updateRaw}
              height="calc(100vh - 250px)"
              ariaLabel={t("MCP configuration JSON", "MCP 配置 JSON")}
            />
          </>
        ) : !server ? (
          <div className="settings-empty">
            <p>{t("No MCP servers yet. Connect stdio, HTTP, or SSE servers to expose tools as mcp_<server>_<tool>.", "还没有 MCP 服务。可接入 stdio / HTTP / SSE，工具会注册为 mcp_<server>_<tool>。")}</p>
            <Button className="settings-secondary" onClick={addServer}>
              <Plus size={14} />{t("Add MCP server", "添加 MCP 服务")}
            </Button>
          </div>
        ) : (
          <>
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

            <McpToolBrowser
              serverId={server.id}
              tools={scannedServerId === server.id ? (scanTools.data?.tools ?? []) : []}
              scanning={scanTools.isPending}
              scanned={scannedServerId === server.id}
              error={scanTools.error ? toDisplayError(scanTools.error, "MCP tool scan failed", "MCP 工具扫描失败").message : null}
              onScan={() => void scanTools.mutateAsync(server).catch(() => undefined)}
            />

            <div className="settings-note-card">
              <Cable size={15} />
              <div>
                <strong>{t("Independent config file", "独立配置文件")}</strong>
                <p>
                  {t(
                    `MCP lives in ${path}. Use the JSON tab to edit the whole file like Advanced settings.`,
                    `MCP 保存在 ${path}。可用 JSON 页签像高级配置一样编辑整份文件。`
                  )}
                </p>
              </div>
            </div>
          </>
        )}
      </section>
    </div>
  );
}

/** 解析 MCP JSON 文本。 */
function parseMcpJson(raw: string): McpConfig {
  const value = JSON.parse(raw) as McpConfig;
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("MCP configuration must be a JSON object");
  }
  if (value.servers !== undefined && !Array.isArray(value.servers)) {
    throw new Error("mcp.servers must be an array");
  }
  return value;
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
