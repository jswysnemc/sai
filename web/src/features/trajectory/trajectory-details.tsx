import { Check, Copy, X } from "lucide-react";
import type { ReactNode } from "react";
import { useCopyAction } from "../chat/tool-renderers/use-copy-action";
import { useI18n } from "../i18n/use-i18n";
import { DetailsBody } from "./details-body";
import { formatClock, formatDuration, formatTokens, prettyJson } from "./trajectory-format";
import { RECORD_KIND_LABELS, recordEndedAt, type TrajectoryRecord } from "./trajectory-record";
import "./trajectory-details.css";

type TrajectoryDetailsProps = {
  record: TrajectoryRecord | null;
  onClose: () => void;
};

/**
 * 渲染选中记录的完整内容。
 *
 * 用量按轮次汇总标注来源：sai 只记录整轮的合计，
 * 把它挂在单条记录上而不说明口径，会被读成这一步的开销。
 *
 * @param props 选中的记录与关闭回调
 * @returns 详情面板；未选中时提示如何使用
 */
export function TrajectoryDetails({ record, onClose }: TrajectoryDetailsProps) {
  const { t, locale } = useI18n();
  const zh = locale.startsWith("zh");

  if (!record) {
    return (
      <aside className="trajectory-details trajectory-details-idle">
        <p>{t("Select a record to inspect its full input, output and timing.", "选中一条记录查看完整入参、输出与计时。")}</p>
      </aside>
    );
  }

  const detail = record.detail;
  const kindLabel = zh ? RECORD_KIND_LABELS[record.kind].zh : RECORD_KIND_LABELS[record.kind].en;
  const endedAt = recordEndedAt(record);

  return (
    <aside className="trajectory-details" aria-label={t("Record details", "记录详情")}>
      <header className="trajectory-details-head">
        <span className="trajectory-details-index">#{record.index}</span>
        <span className="trajectory-details-kind" data-kind={record.kind}>{kindLabel}</span>
        {record.label && <code className="trajectory-details-label">{record.label}</code>}
        <button
          type="button"
          className="trajectory-details-close"
          aria-label={t("Close details", "关闭详情")}
          onClick={onClose}
        >
          <X size={13} aria-hidden />
        </button>
      </header>

      <dl className="trajectory-details-facts">
        <Fact label={t("Started", "开始")} value={formatClock(record.startedAt, locale)} />
        <Fact label={t("Ended", "结束")} value={record.durationMs == null ? "-" : formatClock(endedAt, locale)} />
        <Fact label={t("Duration", "耗时")} value={record.running ? t("running", "运行中") : formatDuration(record.durationMs)} />
        {record.turnSeq != null && (
          <Fact label={t("Turn", "轮次")} value={String(record.turnSeq)} />
        )}
        {record.round > 0 && (
          <Fact label={t("Request", "请求")} value={`#${record.round}`} />
        )}
      </dl>

      {detail.usage && (
        <section className="trajectory-details-section">
          <h4>{t("Turn usage", "本轮用量")}</h4>
          <dl className="trajectory-details-facts">
            <Fact label={t("Input", "输入")} value={formatTokens(detail.usage.prompt_tokens)} />
            <Fact label={t("Output", "输出")} value={formatTokens(detail.usage.completion_tokens)} />
            <Fact label={t("Cache read", "缓存读取")} value={formatTokens(detail.usage.cache_read_tokens)} />
            <Fact label={t("Cache write", "缓存写入")} value={formatTokens(detail.usage.cache_write_tokens)} />
          </dl>
          <p className="trajectory-details-note">
            {t("Usage is recorded per turn, not per record.", "用量按整轮记录，不区分到单条记录。")}
          </p>
        </section>
      )}

      {detail.error && (
        <Block title={t("Error", "错误")} body={detail.error} tone="danger" />
      )}
      {detail.reasoning && (
        <Block title={t("Reasoning", "思考过程")} body={detail.reasoning} />
      )}
      {detail.sections?.length
        ? detail.sections.map((section) => (
            <Block key={section.id} title={section.label} body={section.content} />
          ))
        : detail.input && (
            <Block
              title={record.kind === "tool" ? t("Arguments", "入参") : t("Content", "内容")}
              body={detail.inputIsJson ? prettyJson(detail.input) : detail.input}
            />
          )}
      {detail.actualRequest && (
        <p className="trajectory-details-note">{t("Loaded from the recorded provider request.", "内容来自已记录的真实供应商请求。")}</p>
      )}
      {detail.preview && (
        <p className="trajectory-details-note">{t("This is a current configuration preview, not a historical provider request. Enable API debug to capture the exact request.", "这是当前配置预览，不是历史供应商请求。开启 API 调试后可记录精确请求。")}</p>
      )}
      {detail.output && (
        <Block
          title={t("Output", "输出")}
          body={detail.output}
          footer={detail.originalChars && detail.originalChars > detail.output.length
            ? t(
                `Truncated from ${detail.originalChars} characters${detail.resultRef ? `; full output at ${detail.resultRef}` : ""}`,
                `已从 ${detail.originalChars} 字符截断${detail.resultRef ? `，完整输出见 ${detail.resultRef}` : ""}`
              )
            : undefined}
        />
      )}
      {detail.permission && (
        <section className="trajectory-details-section">
          <h4>{t("Permission", "权限裁决")}</h4>
          <p className="trajectory-details-note">{describePermission(detail.permission, zh)}</p>
        </section>
      )}
    </aside>
  );
}

/**
 * 渲染一项键值事实。
 *
 * @param props 标签与取值
 * @returns 事实项
 */
function Fact({ label, value }: { label: string; value: string }): ReactNode {
  return (
    <div className="trajectory-fact">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/**
 * 渲染一段可复制的长文本。
 *
 * @param props 标题、正文、可选脚注与语气
 * @returns 文本区块
 */
function Block({
  title,
  body,
  footer,
  tone
}: {
  title: string;
  body: string;
  footer?: string;
  tone?: "danger";
}): ReactNode {
  const { t } = useI18n();
  const { copied, copy } = useCopyAction();
  return (
    <section className="trajectory-details-section" data-tone={tone}>
      <h4>
        {title}
        <button
          type="button"
          className="trajectory-details-copy"
          aria-label={t("Copy", "复制")}
          title={t("Copy", "复制")}
          onClick={() => copy(body)}
        >
          {copied ? <Check size={12} aria-hidden /> : <Copy size={12} aria-hidden />}
        </button>
      </h4>
      <DetailsBody title={title} body={body} />
      {footer && <p className="trajectory-details-note">{footer}</p>}
    </section>
  );
}

/**
 * 用一句话描述权限裁决。
 *
 * @param permission 权限裁决记录
 * @param zh 是否为中文界面
 * @returns 裁决描述
 */
function describePermission(permission: NonNullable<TrajectoryRecord["detail"]["permission"]>, zh: boolean): string {
  if (permission.decision === "deny") {
    const reply = permission.reply?.trim();
    return `${zh ? "已拒绝" : "Denied"}${reply ? ` · ${reply}` : ""}`;
  }
  const source = permission.source ? ` · ${permission.source}` : "";
  const reason = permission.reason?.trim();
  return `${zh ? "已允许" : "Allowed"}${source}${reason ? ` · ${reason}` : ""}`;
}
