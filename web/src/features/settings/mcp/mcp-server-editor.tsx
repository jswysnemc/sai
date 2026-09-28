import type { UseMutationResult } from "@tanstack/react-query";
import type { McpServerConfig } from "../../../api/contracts";
import type { McpToolInfo } from "../../../api/mcp-tool-contracts";
import { toDisplayError } from "../../../api/api-error";
import { Button } from "../../../shared/ui/button/button";
import { ChoicePills, EmptyGuide, FieldGrid, SettingsField, SettingsPanel, SkListInput, SkNumberInput, SkTextInput } from "../kit";
import { KeyValueEditor } from "../key-value-editor";
import { McpToolBrowser } from "./mcp-tool-browser";
import { useI18n } from "../../i18n/use-i18n";

type Props = {
  server: McpServerConfig | undefined;
  selectedIndex: number;
  path: string;
  scannedServerId: string;
  scanTools: UseMutationResult<{ tools: McpToolInfo[] }, Error, McpServerConfig, unknown>;
  onUpdateServer: (index: number, patch: Partial<McpServerConfig>) => void;
  onAddServer: () => void;
};

/**
 * 【MCP】【服务编辑】编辑传输参数并在当前服务下展示发现结果。
 * @param props 服务、扫描状态与草稿更新回调
 * @returns 服务表单与工具目录
 */
export function McpServerEditor({ server, selectedIndex, scannedServerId, scanTools, onUpdateServer, onAddServer }: Props) {
  const { t } = useI18n();
  if (!server) return <EmptyGuide title={t("No MCP servers configured", "尚未配置 MCP 服务")} description={t("Connect a local process or a remote HTTP/SSE endpoint.", "可连接本地进程或远程 HTTP/SSE 端点。")} action={<Button variant="secondary" onClick={onAddServer}>{t("Add MCP server", "添加 MCP 服务")}</Button>} />;
  const transport = server.transport ?? "stdio";
  /** 合并当前服务字段；参数为补丁，返回无值。 */
  const update = (patch: Partial<McpServerConfig>) => onUpdateServer(selectedIndex, patch);
  const ownScan = JSON.stringify(scanTools.variables) === JSON.stringify(server);
  return <>
    <SettingsPanel title={t("Connection", "连接")}>
      <FieldGrid>
        <SettingsField label={t("Server ID", "服务 ID")} anchor="mcp.id" hint={t("Used in exposed tool names.", "用于暴露的工具名称。")}><SkTextInput value={server.id} onChange={(id) => update({ id: id.trim() || server.id })} /></SettingsField>
        <SettingsField label={t("Transport", "传输方式")} anchor="mcp.transport"><ChoicePills value={transport} options={[{ value: "stdio", label: "stdio" }, { value: "http", label: "HTTP" }, { value: "sse", label: "SSE" }]} onChange={(value) => update({ transport: value })} /></SettingsField>
        <SettingsField label={t("Timeout", "超时")} anchor="mcp.timeout_ms" size="sm" hint={t("Request or process startup timeout.", "请求或进程启动超时。")}><SkNumberInput value={server.timeout_ms ?? 30000} min={100} max={300000} unit="ms" onChange={(timeout_ms) => update({ timeout_ms })} /></SettingsField>
        {transport === "stdio" ? <>
          <SettingsField label={t("Command", "命令")} anchor="mcp.command" hint={t("Executable on PATH, such as npx, uvx or node.", "PATH 中的可执行文件，如 npx、uvx 或 node。")}><SkTextInput mono value={server.command ?? ""} onChange={(command) => update({ command })} /></SettingsField>
          <SettingsField label={t("Working directory", "工作目录")} anchor="mcp.cwd"><SkTextInput mono value={server.cwd ?? ""} onChange={(cwd) => update({ cwd: cwd || null })} /></SettingsField>
          <SettingsField label={t("Arguments", "参数")} anchor="mcp.args" hint={t("One argument per line; spaces within an argument are preserved.", "每行一个参数，保留参数内部空格。")}><SkListInput value={server.args ?? []} onChange={(args) => update({ args })} /></SettingsField>
          <SettingsField label={t("Environment", "环境变量")} anchor="mcp.env" span="full"><KeyValueEditor value={server.env ?? {}} keyPlaceholder={t("Variable name", "变量名")} valuePlaceholder={t("Value", "值")} addLabel={t("Add variable", "添加变量")} onChange={(env) => update({ env })} /></SettingsField>
        </> : <>
          <SettingsField label="URL" anchor="mcp.url" span="full"><SkTextInput mono value={server.url ?? ""} onChange={(url) => update({ url: url || null })} /></SettingsField>
          {transport === "sse" && <SettingsField label={t("Message address", "消息地址")} anchor="mcp.message_url" span="full" hint={t("Optional; read from the SSE endpoint event when empty.", "可选；留空时从 SSE 端点事件读取。")}><SkTextInput mono value={server.message_url ?? ""} onChange={(message_url) => update({ message_url: message_url || null })} /></SettingsField>}
          <SettingsField label={t("Headers", "请求头")} anchor="mcp.headers" span="full"><KeyValueEditor value={server.headers ?? {}} keyPlaceholder={t("Header name", "请求头名称")} valuePlaceholder={t("Header value", "请求头值")} addLabel={t("Add header", "添加请求头")} onChange={(headers) => update({ headers })} /></SettingsField>
        </>}
      </FieldGrid>
    </SettingsPanel>
    <McpToolBrowser key={server.id} serverId={server.id} tools={ownScan && scannedServerId === server.id ? (scanTools.data?.tools ?? []) : []} scanning={ownScan && scanTools.isPending} scanned={ownScan && scannedServerId === server.id} error={ownScan && scanTools.error ? toDisplayError(scanTools.error, "MCP tool scan failed", "MCP 工具扫描失败").message : null} onScan={() => scanTools.mutate(server)} />
  </>;
}
