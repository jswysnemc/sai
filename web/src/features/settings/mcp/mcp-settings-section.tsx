import { Globe2, Save, Terminal } from "../../../shared/ui/icons";
import { toDisplayError } from "../../../api/api-error";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { JsonCodeEditor } from "../../../shared/ui/code-editor/json-code-editor";
import { Button } from "../../../shared/ui/button/button";
import { ChoicePills, DetailHeader, InlineSwitch, MasterDetail, ObjectList, SwitchField } from "../kit";
import { useI18n } from "../../i18n/use-i18n";
import { transportMeta } from "./mcp-helpers";
import { McpServerEditor } from "./mcp-server-editor";
import { useMcpConfig } from "./use-mcp-config";

/**
 * 【MCP】【配置】组合独立配置文件的表单、JSON 编辑与保存。
 * @returns MCP 设置页
 */
export function McpSettingsSection() {
  const { t } = useI18n();
  const confirm = useConfirm();
  const state = useMcpConfig();
  const { loading, path, loadError, mcp, raw, dirty, mode, selectedId, selectedIndex, server, servers, parseError, setParseError, save, setSelectedId, patchMcp, updateServer, addServer, removeServerAt, switchMode, updateRaw } = state;
  if (loadError) return <div className="settings-inline-error">{toDisplayError(loadError, "MCP configuration error", "MCP 配置错误").message}</div>;
  if (loading || !mcp) return <div className="settings-state">{t("Loading MCP configuration", "正在读取 MCP 配置")}</div>;
  /** 确认删除当前服务；返回操作完成的 Promise。 */
  const remove = async () => {
    if (server && await confirm({ title: t("Delete MCP server", "删除 MCP 服务"), description: t(`Delete “${server.id}” and stop exposing its tools.`, `删除“${server.id}”，其工具将不再暴露。`), confirmLabel: t("Delete", "删除"), danger: true })) removeServerAt(selectedIndex);
  };
  const editor = <>
    <DetailHeader title={mode === "json" ? "MCP JSON" : server?.id ?? t("Server configuration", "服务配置")} subtitle={path} subtitleMono actions={<>
      <ChoicePills value={mode} onChange={switchMode} ariaLabel={t("MCP editor mode", "MCP 编辑模式")} options={[{ value: "form", label: t("Form", "表单") }, { value: "json", label: "JSON" }]} />
      <Button variant="secondary" disabled={!dirty || save.isPending || Boolean(parseError)} onClick={() => void save.mutateAsync().catch((cause) => setParseError(cause instanceof Error ? cause.message : String(cause)))}><Save size={14} />{save.isPending ? t("Saving", "正在保存") : t("Save MCP", "保存 MCP")}</Button>
      {mode === "form" && server && <InlineSwitch label={t("Enabled", "已启用")} checked={server.enabled !== false} onChange={(enabled) => updateServer(selectedIndex, { enabled })} />}
    </>} menuItems={mode === "form" && server ? [{ id: "delete", label: t("Delete server", "删除服务"), danger: true, onSelect: () => void remove() }] : []} />
    {(save.error || parseError) && <div className="settings-inline-error">{parseError ?? toDisplayError(save.error, "MCP save error", "MCP 保存错误").message}</div>}
    {mode === "json" ? <JsonCodeEditor value={raw} onChange={updateRaw} height="calc(100dvh - 12rem)" ariaLabel={t("MCP configuration JSON", "MCP 配置 JSON")} /> : <McpServerEditor {...state} onUpdateServer={updateServer} onAddServer={addServer} />}
  </>;
  return mode === "json" || !servers.length ? <div className="grid gap-4"><SwitchField label={t("Enable MCP", "启用 MCP")} anchor="mcp.enabled" checked={mcp.enabled !== false} onChange={(enabled) => patchMcp({ enabled })} />{editor}</div> : <MasterDetail list={<ObjectList title="MCP" items={servers.map((item) => ({ id: item.id, name: item.id, meta: transportMeta(item.transport ?? "stdio", item, t), icon: (item.transport ?? "stdio") === "stdio" ? <Terminal size={14} /> : <Globe2 size={14} />, marked: item.enabled !== false }))} selectedId={selectedId} onSelect={setSelectedId} onAdd={addServer} searchPlaceholder={t("Search MCP servers", "搜索 MCP 服务")} headerSlot={<SwitchField label={t("Enable MCP", "启用 MCP")} anchor="mcp.enabled" checked={mcp.enabled !== false} onChange={(enabled) => patchMcp({ enabled })} hint={t("Save changes with Save MCP.", "修改后使用“保存 MCP”提交。")} />} />}>{editor}</MasterDetail>;
}
