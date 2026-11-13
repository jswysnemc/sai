import { useEffect, useState, type ReactNode } from "react";
import { apiRequest } from "../../../api/api-request";
import type { HookItem } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Collapse } from "../../../shared/ui/collapse/collapse";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsPanel } from "../kit";

type Source = { path: string; digest: string; trusted: boolean; managed: boolean; items: HookItem[]; error?: string };
type Record = { name: string; event: string; source: string; status: string; duration_ms: number; timestamp: number; error?: string; output: string[] };

/** 【Hooks】【条目展开】参数为标题与详情，返回复用统一按钮和折叠容器的紧凑条目。 */
function RuntimeEntry({ title, children }: { title: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return <div className="border-b border-border py-2 text-sm">
    <Button variant="ghost" size="small" className="max-w-full whitespace-normal break-all text-left" aria-expanded={open} onClick={() => setOpen((value) => !value)}>{title}</Button>
    <Collapse open={open}>{children}</Collapse>
  </div>;
}

/** 【Hooks】【运行状态】无参数，返回来源审查和最近执行记录，不主动运行钩子。 */
export function HooksRuntimePanel() {
  const { t } = useI18n();
  const confirm = useConfirm();
  const [sources, setSources] = useState<Source[]>([]);
  const [records, setRecords] = useState<Record[]>([]);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let active = true;
    /** 【Hooks】【状态查询】无参数，返回查询结果，卸载后不再修改状态。 */
    const load = async () => {
      try {
        const [sources, records] = await Promise.all([
          apiRequest<Source[]>("/api/hooks/sources"), apiRequest<Record[]>("/api/hooks/history"),
        ]);
        if (active) { setSources(sources); setRecords(records); setError(""); }
      } catch (error) { if (active) setError(error instanceof Error ? error.message : String(error)); }
    };
    void load();
    const timer = setInterval(() => void load(), 5000);
    return () => { active = false; clearInterval(timer); };
  }, [revision]);
  /** 【Hooks】【确认来源】参数为预览来源，返回授权结果，摘要变化时后端拒绝。 */
  const trust = async (source: Source) => {
    if (!await confirm({ title: t("Trust hook source", "授权钩子来源"), description: t(`Execute the reviewed definitions in ${source.path}? Changed content requires another review.`, `允许执行 ${source.path} 中已经审查的定义？内容变化后需要重新审查。`), confirmLabel: t("Trust reviewed content", "授权已审查内容") })) return;
    try {
      await apiRequest("/api/hooks/trust", { method: "POST", body: JSON.stringify({ source: source.path, digest: source.digest }) });
      setRevision((value) => value + 1);
    } catch (error) { setError(error instanceof Error ? error.message : String(error)); }
  };
  return <SettingsPanel title={t("Sources and recent executions", "来源与最近执行")}>
    {error && <p className="text-sm text-muted">{error}</p>}
    {!sources.length && <p className="text-sm text-muted">{t("Optional sources: user hooks.json, project .sai/hooks.json and enabled installed plugins.", "可选来源包括用户 hooks.json、项目 .sai/hooks.json 与已启用的安装插件。")}</p>}
    {sources.map((source) => <RuntimeEntry key={source.path} title={`${source.path} · ${source.managed ? t("Managed", "托管") : source.trusted ? t("Trusted", "已授权") : t("Review required", "需要审查")}`}>
      <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all py-2 text-xs">{JSON.stringify(source.items, null, 2)}</pre>
      <p className="break-all text-xs text-muted">{source.digest}</p>
      {source.error && <p className="break-all text-xs text-muted">{source.error}</p>}
      {!source.trusted && !source.managed && !source.error && <Button variant="secondary" onClick={() => void trust(source)}>{t("Trust reviewed content", "授权已审查内容")}</Button>}
    </RuntimeEntry>)}
    {records.slice(0, 12).map((record, index) => <RuntimeEntry key={`${record.timestamp}:${index}`} title={`${record.name} · ${record.event} · ${record.status} · ${record.duration_ms} ms`}>
      <p className="break-all text-xs text-muted">{record.source} · {new Date(record.timestamp).toLocaleString()}</p>
      {(record.error || record.output.length > 0) && <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all text-xs">{record.error ?? record.output.join("\n")}</pre>}
    </RuntimeEntry>)}
  </SettingsPanel>;
}
