import { RefreshCw } from "../../../shared/ui/icons";
import { useState } from "react";
import type { McpToolInfo } from "../../../api/mcp-tool-contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { MasterDetail, ObjectList, SettingsPanel } from "../kit";

type Props = { serverId: string; tools: McpToolInfo[]; scanning: boolean; scanned: boolean; error: string | null; onScan: () => void };

/**
 * 【MCP】【工具发现】展示可筛选、可折叠的工具目录和完整输入结构。
 * @param props 服务标识、发现结果、请求状态与扫描回调
 * @returns 工具目录面板
 */
export function McpToolBrowser({ serverId, tools, scanning, scanned, error, onScan }: Props) {
  const { t } = useI18n();
  const [selectedName, setSelectedName] = useState("");
  const selected = tools.find((tool) => tool.name === selectedName) ?? tools[0];
  return <SettingsPanel title={t("Discovered tools", "已发现工具")} collapsible actions={<Button variant="secondary" onClick={onScan} disabled={scanning}><RefreshCw size={14} />{scanning ? t("Scanning", "正在扫描") : t("Scan tools", "扫描工具")}</Button>}>
    {error && <div className="settings-inline-error">{error}</div>}
    {!scanned && !scanning ? <p className="text-xs text-muted">{t(`Scan ${serverId} to load its tool catalog.`, `扫描 ${serverId} 以读取工具目录。`)}</p> : !tools.length ? <p className="text-xs text-muted">{scanning ? t("Loading tools", "正在读取工具") : t("The server returned no tools.", "服务未返回工具。")}</p> : <MasterDetail list={<ObjectList title={t("Tools", "工具")} items={tools.map((tool) => ({ id: tool.name, name: tool.name, meta: tool.description }))} selectedId={selected?.name ?? ""} onSelect={setSelectedName} searchPlaceholder={t("Search tools", "搜索工具")} />}>
      {selected && <div className="grid min-w-0 gap-2 text-xs">
        <strong className="break-all">{selected.name}</strong>
        <p className="whitespace-pre-wrap break-words">{selected.description}</p>
        <pre className="max-h-64 overflow-auto rounded border border-[var(--line)] p-2 text-xs">{JSON.stringify(selected.input_schema ?? {}, null, 2)}</pre>
      </div>}
    </MasterDetail>}
  </SettingsPanel>;
}
