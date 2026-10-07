import { ChevronRight, Layers } from "../../../shared/ui/icons";
import { useEffect, useState } from "react";
import { ReasoningBlock } from "../reasoning-block";
import { PermissionRequestCard } from "../../permission/permission-request-card";
import { SshSecretCard } from "../../ssh/ssh-secret-card";
import { usePersistedExpand } from "./tool-expand-state";
import { ToolWave } from "./tool-wave";
import { useToolWaveQueue } from "./tool-wave-queue";
import { collectWavePermissions, collectWaveSecrets, collectWaveTools, countWorkItems, type WorkItem } from "./group-activity-parts";
import "./activity-stream.css";
import { useI18n } from "../../i18n/use-i18n";

type TranslateFn = ReturnType<typeof useI18n>["t"];

/**
 * 把工作组统计渲染成折叠态标题的副文本。
 *
 * 优先展示「写入 N · 读取 M · 命令 K」这种带动作感的组合，
 * 当所有工具都属于 other 类别时退化为「工具 N」。
 *
 * @param counts 工作组统计
 * @param t 翻译函数
 * @returns 副文本，无任何统计时返回空串
 */
function buildWorkDetail(
  counts: { reasoning: number; tools: number; read: number; write: number; command: number; other: number },
  t: TranslateFn
): string {
  const segments: string[] = [];
  if (counts.reasoning > 0) {
    segments.push(t(`${counts.reasoning} thoughts`, `思考 ${counts.reasoning}`));
  }
  if (counts.write > 0) {
    segments.push(t(`${counts.write} writes`, `写入 ${counts.write}`));
  }
  if (counts.read > 0) {
    segments.push(t(`${counts.read} reads`, `读取 ${counts.read}`));
  }
  if (counts.command > 0) {
    segments.push(t(`${counts.command} commands`, `命令 ${counts.command}`));
  }
  if (counts.other > 0) {
    segments.push(t(`${counts.other} others`, `其他 ${counts.other}`));
  }
  return segments.join(" · ");
}

/**
 * 把正文前的连续思考与工具收成一组，完成后默认折叠。
 *
 * @param props 工作组条目、是否后接正文、是否处于实时运行
 * @returns 可折叠的工作组
 */
export function ActivityPreamble({
  id,
  items,
  followedByText,
  live
}: {
  id: string;
  items: WorkItem[];
  followedByText: boolean;
  live?: boolean;
}) {
  const { t } = useI18n();
  const counts = countWorkItems(items);
  const tools = collectWaveTools(items);
  const permissions = collectWavePermissions(items);
  const secrets = collectWaveSecrets(items);
  const pendingSecret = secrets.some((part) => !part.resolved);
  const working = Boolean(live) && tools.some((part) => (
    part.tool.status === "preparing" || part.tool.status === "running"
  ));
  const defaultOpen = Boolean(live) && (!followedByText || pendingSecret);
  const [open, setOpen] = usePersistedExpand(id, defaultOpen);
  const [userToggled, setUserToggled] = useState(false);
  const snapshots = tools.map((part) => ({ id: part.tool.id, status: part.tool.status }));
  const wave = useToolWaveQueue(snapshots, !open && snapshots.length > 1);
  const collapsedShowsWave = !open && counts.tools > 0 && (working || wave.busy);

  useEffect(() => {
    if (pendingSecret) {
      setOpen(true);
      return;
    }
    if (userToggled) return;
    setOpen(defaultOpen);
  }, [defaultOpen, pendingSecret, setOpen, userToggled]);

  const label = working
    ? t("Working", "正在处理")
    : t("Work log", "处理过程");
  const detail = buildWorkDetail(counts, t);

  return (
    <section className={`activity-preamble${open ? " is-open" : ""}`}>
      <button
        type="button"
        className="activity-preamble-head"
        aria-expanded={open}
        onClick={() => {
          setUserToggled(true);
          setOpen((value) => !value);
        }}
      >
        <span className="activity-preamble-icon" aria-hidden><Layers size={14} /></span>
        <span className="activity-preamble-label">{detail ? `${label} · ${detail}` : label}</span>
        <ChevronRight size={14} />
      </button>
      {open ? (
        <div className="activity-preamble-body">
          {items.map((item) => (
            item.kind === "reasoning" ? (
              <ReasoningBlock
                key={item.part.id}
                source={item.part.source}
                live={live && !item.part.endedAt}
                startedAt={item.part.startedAt}
                endedAt={item.part.endedAt}
              />
            ) : (
              <ToolWave
                key={item.parts[0]?.id ?? "wave"}
                parts={item.parts}
                live={live}
              />
            )
          ))}
        </div>
      ) : (
        permissions.length > 0 || secrets.length > 0 || collapsedShowsWave ? (
          <div className="activity-preamble-collapsed">
            {permissions.map((part) => (
              <PermissionRequestCard
                key={part.id}
                request={part.request}
                decision={part.decision}
                active={Boolean(live)}
              />
            ))}
            {secrets.map((part) => (
              <SshSecretCard
                key={part.id}
                request={part.request}
                resolved={part.resolved}
                active={Boolean(live)}
              />
            ))}
            {collapsedShowsWave ? (
              <ToolWave
                parts={tools}
                live={live}
                preferCarousel
                activeId={wave.currentId}
                carouselActive={wave.busy}
                onExpand={() => {
                  setUserToggled(true);
                  setOpen(true);
                }}
              />
            ) : null}
          </div>
        ) : null
      )}
    </section>
  );
}
