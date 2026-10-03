import type { ToolLifecycle } from "../run-event-reducer";
import { useI18n } from "../../i18n/use-i18n";
import { ToolLayout } from "./layout/tool-layout";
import { ToolPanel } from "./layout/tool-panel";
import { ToolIcon } from "./tool-icon";
import { parseJsonRecord, stringField } from "./tool-data";

/**
 * 【上下文】【压缩反馈】显示局部摘要的生成、提交、结果与失败原因。
 * @param props 工具生命周期、展开状态及切换回调
 * @returns 复用工具外壳的压缩反馈卡片
 */
export function ContextCompressionTool({ tool, expanded, onToggle }: {
  tool: ToolLifecycle;
  expanded: boolean;
  onToggle: () => void;
}) {
  const { t, locale } = useI18n();
  const args = parseJsonRecord(tool.arguments || tool.argumentsPreview);
  const receipt = parseJsonRecord(tool.output);
  const before = receipt?.before_tokens;
  const after = receipt?.after_tokens;
  const hasCounts = typeof before === "number" && Number.isSafeInteger(before) && before >= 0
    && typeof after === "number" && Number.isSafeInteger(after) && after >= 0;
  const running = tool.status === "preparing" || tool.status === "running";
  const failed = tool.status === "failed";
  const label = tool.status === "preparing"
    ? t("Preparing context summary", "正在生成上下文摘要")
    : tool.status === "running" ? t("Compressing context", "正在压缩上下文")
      : failed ? t("Context compression failed", "上下文压缩失败")
        : t("Context compressed", "上下文已压缩");
  const counts = !running && !failed && hasCounts
    ? `${before.toLocaleString(locale)} → ${after.toLocaleString(locale)} token`
    : undefined;
  const summary = stringField(args, "summary");

  return (
    <div role="status" aria-live="polite" className="min-w-0 text-[0.8125rem] sm:text-[0.875rem]">
      <ToolLayout
        icon={<ToolIcon name="compress_context" />}
        kindLabel={label}
        primaryText={stringField(args, "topic")}
        statusLabel={counts}
        showFailureStatus={failed}
        isRunning={running}
        expanded={expanded}
        onToggle={onToggle}
      >
        <ToolPanel>
          <div className="max-h-60 space-y-2 overflow-auto p-2 break-words">
            {failed ? <p className="whitespace-pre-wrap">{tool.output || t("Compression was not applied.", "未应用本次压缩。")}</p>
              : <>
                {summary && <p className="whitespace-pre-wrap">{summary}</p>}
                {!running && <p>{t("Original tool outputs remain available for retrieval.", "工具原文已保留，可按需回读。")}</p>}
                {counts && <p className="text-muted-foreground">{t("Counts estimate selected result text only, not billing savings.", "数量仅估算所选结果正文，不代表账单节省。")}</p>}
              </>}
          </div>
        </ToolPanel>
      </ToolLayout>
    </div>
  );
}
