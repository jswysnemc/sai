import { Braces, FormInput, Globe2, Save, Terminal, Trash2 } from "lucide-react";
import { toDisplayError } from "../../../api/api-error";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { JsonCodeEditor } from "../../../shared/ui/code-editor/json-code-editor";
import { Button } from "../../../shared/ui/button/button";
import { EditorHeader } from "../editor-layout";
import { ObjectListPanel } from "../object-list-panel";
import { useI18n } from "../../i18n/use-i18n";
import { transportMeta } from "./mcp-helpers";
import { McpServerEditor } from "./mcp-server-editor";
import { useMcpConfig } from "./use-mcp-config";

/**
 * 渲染独立 MCP 配置（`~/.config/sai/mcp.jsonc`）。
 *
 * 支持结构化表单和完整 JSON 两种编辑方式，保存写入独立配置文件，不走顶栏 AppConfig Save。
 *
 * @returns MCP 配置区域
 */
export function McpSettingsSection() {
  const { t } = useI18n();
  const confirm = useConfirm();
  const mcpState = useMcpConfig();
  const {
    loading,
    path,
    loadError,
    mcp,
    raw,
    dirty,
    mode,
    selectedId,
    selectedIndex,
    server,
    servers,
    parseError,
    setParseError,
    scannedServerId,
    save,
    scanTools,
    setSelectedId,
    patchMcp,
    updateServer,
    addServer,
    removeServerAt,
    switchMode,
    updateRaw
  } = mcpState;

  if (loading || !mcp) {
    return <div className="settings-state">{t("Loading MCP configuration", "正在读取 MCP 配置")}</div>;
  }

  const error = (loadError ?? save.error)
    ? toDisplayError(loadError ?? save.error, "MCP configuration error", "MCP 配置错误")
    : null;

  const deleteServer = async () => {
    if (!server) return;
    const confirmed = await confirm({
      title: t("Delete MCP server", "删除 MCP 服务"),
      description: t(`Delete “${server.id}” and stop exposing its tools.`, `删除“${server.id}”，其工具将不再暴露。`),
      confirmLabel: t("Delete server", "删除服务"),
      danger: true
    });
    if (!confirmed) return;
    removeServerAt(selectedIndex);
  };

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
      <Save size={14} />
      {save.isPending ? t("Saving", "正在保存") : dirty ? t("Save MCP", "保存 MCP") : t("Saved", "已保存")}
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
            `File: ${path}. Form and JSON edit the same independent config. Top-bar Save does not apply here.`,
            `文件：${path}。表单与 JSON 编辑同一份独立配置。顶栏保存不作用于本节。`
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
        ) : (
          <McpServerEditor
            server={server}
            selectedIndex={selectedIndex}
            path={path}
            scannedServerId={scannedServerId}
            scanTools={scanTools}
            onUpdateServer={updateServer}
            onAddServer={addServer}
          />
        )}
      </section>
    </div>
  );
}
