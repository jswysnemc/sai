import { Braces, RefreshCw, Wrench } from "lucide-react";
import { useEffect, useState } from "react";
import type { McpToolInfo } from "../../api/mcp-tool-contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import "./mcp-tool-browser.css";

type McpToolBrowserProps = {
  serverId: string;
  tools: McpToolInfo[];
  scanning: boolean;
  scanned: boolean;
  error: string | null;
  onScan: () => void;
};

/**
 * 展示 MCP 工具扫描结果及完整输入参数结构。
 *
 * @param props 服务标识、工具结果、扫描状态与触发回调
 * @returns MCP 工具浏览器
 */
export function McpToolBrowser({ serverId, tools, scanning, scanned, error, onScan }: McpToolBrowserProps) {
  const { t } = useI18n();
  const [selectedName, setSelectedName] = useState("");

  useEffect(() => {
    if (!tools.some((tool) => tool.name === selectedName)) {
      setSelectedName(tools[0]?.name ?? "");
    }
  }, [selectedName, tools]);

  const selected = tools.find((tool) => tool.name === selectedName) ?? null;
  return (
    <section className="mcp-tool-browser">
      <header className="mcp-tool-browser-head">
        <div>
          <strong>{t("Discovered tools", "已发现工具")}</strong>
          <small>{t("Inspect descriptions and input JSON Schema returned by the server.", "查看服务返回的说明与输入 JSON Schema。")}</small>
        </div>
        <Button className="settings-secondary" onClick={onScan} disabled={scanning}>
          <RefreshCw size={14} className={scanning ? "is-spinning" : ""} />
          {scanning ? t("Scanning", "正在扫描") : t("Scan tools", "扫描工具")}
        </Button>
      </header>

      {error && <div className="settings-inline-error">{error}</div>}
      {!scanned && !scanning ? (
        <div className="mcp-tool-empty"><Wrench size={18} /><span>{t(`Scan ${serverId} to load its tool catalog.`, `扫描 ${serverId} 以读取工具目录。`)}</span></div>
      ) : scanned && tools.length === 0 ? (
        <div className="mcp-tool-empty"><Wrench size={18} /><span>{t("The server returned no tools.", "服务未返回工具。")}</span></div>
      ) : (
        <div className="mcp-tool-browser-body">
          <nav className="mcp-tool-list" aria-label={t("MCP tools", "MCP 工具列表")}>
            {tools.map((tool) => (
              <button
                type="button"
                key={tool.name}
                className={tool.name === selectedName ? "active" : ""}
                onClick={() => setSelectedName(tool.name)}
              >
                <Wrench size={13} />
                <span><strong>{tool.name}</strong><small>{tool.description || t("No description", "无说明")}</small></span>
              </button>
            ))}
          </nav>
          {selected && (
            <article className="mcp-tool-detail">
              <div className="mcp-tool-detail-title">
                <span><Wrench size={14} />{selected.name}</span>
                <code>mcp_{selected.server_id}_{selected.name}</code>
              </div>
              <p>{selected.description || t("This tool does not provide a description.", "此工具未提供说明。")}</p>
              <div className="mcp-tool-schema-title"><Braces size={13} />{t("Input schema", "输入参数结构")}</div>
              <pre>{JSON.stringify(selected.input_schema ?? {}, null, 2)}</pre>
            </article>
          )}
        </div>
      )}
    </section>
  );
}
