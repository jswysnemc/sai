import { ToolLifecycleCard } from "../tool-lifecycle-card";
import { PermissionRequestCard } from "../../permission/permission-request-card";
import { SshSecretCard } from "../../ssh/ssh-secret-card";
import { splitWaveForEarlyReadFold, type WavePart } from "./group-activity-parts";
import { ReadFold } from "./read-fold";
import "./activity-stream.css";

/**
 * 渲染一轮连续工具：折叠且并行时按队列轮播当前条目，组展开后才逐条列出。
 *
 * @param props 本轮工具/权限部件与实时标记
 * @returns 轮播、折叠占位或展开后的工具列表
 */
export function ToolWave({
  parts,
  live,
  preferCarousel,
  activeId,
  carouselActive,
  onExpand
}: {
  parts: WavePart[];
  live?: boolean;
  preferCarousel?: boolean;
  activeId?: string | null;
  carouselActive?: boolean;
  onExpand?: () => void;
}) {
  const tools = parts.filter((part) => part.type === "tool");
  const parallel = tools.length > 1;
  const carousel = Boolean(preferCarousel) && parallel && Boolean(carouselActive);

  /**
   * 展开堆叠：优先交给外层工作组。
   *
   * @returns 无
   */
  const expand = () => {
    onExpand?.();
  };

  if (preferCarousel && parallel && !carousel) return null;

  if (!carousel) {
    return (
      <div className="tool-wave-stack">
        {splitWaveForEarlyReadFold(parts).map((segment) => {
          if (segment.kind === "readFold") {
            return <ReadFold key={`${segment.parts[0]?.id}-${segment.parts[segment.parts.length - 1]?.id}`} parts={segment.parts} />;
          }
          return segment.parts.map((part) => renderWavePart(part, live));
        })}
      </div>
    );
  }

  const activeIndex = Math.max(0, tools.findIndex((part) => part.tool.id === activeId));
  const current = tools[activeIndex] ?? tools[0];
  return (
    <div
      className="tool-wave-carousel"
      role="button"
      tabIndex={0}
      aria-label={`${activeIndex + 1} / ${tools.length}`}
      onClick={expand}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          expand();
        }
      }}
    >
      <div className="tool-wave-viewport">
        <div key={`${current.id}:${current.tool.status}`} className="tool-wave-slide">
          <ToolLifecycleCard
            tool={current.tool}
            batchLabel={`${activeIndex + 1}/${tools.length}`}
          />
        </div>
      </div>
    </div>
  );
}

/**
 * 渲染波次中的单条工具、权限或 SSH 输入卡。
 *
 * @param part 波次部件
 * @param live 是否处于实时运行
 * @returns 对应卡片
 */
function renderWavePart(part: WavePart, live?: boolean) {
  if (part.type === "tool") return <ToolLifecycleCard key={part.id} tool={part.tool} />;
  if (part.type === "permission") {
    return (
      <PermissionRequestCard
        key={part.id}
        request={part.request}
        decision={part.decision}
        active={Boolean(live)}
      />
    );
  }
  return (
    <SshSecretCard
      key={part.id}
      request={part.request}
      resolved={part.resolved}
      active={Boolean(live)}
    />
  );
}
