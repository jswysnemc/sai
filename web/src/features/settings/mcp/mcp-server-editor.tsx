import type { UseMutationResult } from "@tanstack/react-query";
import { Cable, Plus } from "lucide-react";
import type { McpServerConfig } from "../../../api/contracts";
import type { McpToolInfo } from "../../../api/mcp-tool-contracts";
import { toDisplayError } from "../../../api/api-error";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import { SettingsGroup } from "../editor-layout";
import { KeyValueEditor } from "../key-value-editor";
import { McpToolBrowser } from "./mcp-tool-browser";
import { useI18n } from "../../i18n/use-i18n";

type McpServerEditorProps = {
  server: McpServerConfig | undefined;
  selectedIndex: number;
  path: string;
  scannedServerId: string;
  scanTools: UseMutationResult<{ tools: McpToolInfo[] }, Error, McpServerConfig, unknown>;
  onUpdateServer: (index: number, patch: Partial<McpServerConfig>) => void;
  onAddServer: () => void;
};

/**
 * 渲染单个 MCP 服务的结构化表单与工具扫描。
 *
 * @param props 当前服务与更新回调
 * @returns 服务编辑区
 */
export function McpServerEditor({
  server,
  selectedIndex,
  path,
  scannedServerId,
  scanTools,
  onUpdateServer,
  onAddServer
}: McpServerEditorProps) {
  const { t } = useI18n();
  if (!server) {
    return (
      <div className="settings-empty">
        <p>
          {t(
            "No MCP servers yet. Connect stdio, HTTP, or SSE servers to expose tools as mcp_<server>_<tool>.",
            "还没有 MCP 服务。可接入 stdio / HTTP / SSE，工具会注册为 mcp_<server>_<tool>。"
          )}
        </p>
        <Button className="settings-secondary" onClick={onAddServer}>
          <Plus size={14} />
          {t("Add MCP server", "添加 MCP 服务")}
        </Button>
      </div>
    );
  }

  const transport = server.transport ?? "stdio";
  const transportOptions = [
    { value: "stdio", label: t("stdio (local process)", "stdio（本地进程）") },
    { value: "http", label: "HTTP" },
    { value: "sse", label: "SSE" }
  ];

  return (
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
              onChange={(event) => onUpdateServer(selectedIndex, { id: event.target.value.trim() || server.id })}
              spellCheck={false}
            />
            <small>{t("Used in tool names: mcp_<id>_<tool>", "会出现在工具名：mcp_<id>_<tool>")}</small>
          </label>
          <div className="settings-field">
            <span>{t("Transport", "传输方式")}</span>
            <Select
              value={transport}
              options={transportOptions}
              onChange={(value) => onUpdateServer(selectedIndex, { transport: value })}
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
              onChange={(event) => onUpdateServer(selectedIndex, {
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
                onChange={(event) => onUpdateServer(selectedIndex, { command: event.target.value })}
                spellCheck={false}
                placeholder="npx"
              />
              <small>{t("Executable on PATH, e.g. npx / uvx / node", "PATH 上的可执行文件，如 npx / uvx / node")}</small>
            </label>
            <label className="settings-field">
              <span>{t("Working directory", "工作目录")}</span>
              <input
                value={server.cwd ?? ""}
                onChange={(event) => onUpdateServer(selectedIndex, { cwd: event.target.value || null })}
                spellCheck={false}
                placeholder={t("Optional; defaults to workspace", "可选；默认工作区")}
              />
            </label>
            <label className="settings-field full">
              <span>{t("Arguments", "参数")}</span>
              <textarea
                rows={3}
                value={(server.args ?? []).join("\n")}
                onChange={(event) => onUpdateServer(selectedIndex, {
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
                onChange={(env) => onUpdateServer(selectedIndex, { env })}
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
                onChange={(event) => onUpdateServer(selectedIndex, { url: event.target.value || null })}
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
                  onChange={(event) => onUpdateServer(selectedIndex, { message_url: event.target.value || null })}
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
                onChange={(headers) => onUpdateServer(selectedIndex, { headers })}
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
              `MCP lives in ${path}. Save with the section button; top-bar AppConfig Save does not write this file.`,
              `MCP 保存在 ${path}。请用本节保存按钮；顶栏 AppConfig 保存不会写入此文件。`
            )}
          </p>
        </div>
      </div>
    </>
  );
}
