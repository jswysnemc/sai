import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import { groupCompletedToolCalls, type GroupedMessagePart } from "./tool-call-grouping";

/** 思考与其后连续工具调用构成的一个决策-执行步骤 */
export type StepGroup = {
  type: "step";
  id: string;
  /** 该步骤的思考部件 */
  reasoning: Extract<LiveMessagePart, { type: "reasoning" }>;
  /** 思考之后紧邻的已完成工具 */
  tools: ToolLifecycle[];
};

/** 渲染层最终消费的部件：步骤组、工具组或单个部件 */
export type StepMessagePart = GroupedMessagePart | StepGroup;

/**
 * 将「一段思考 + 其后连续工具调用」聚合为步骤组。
 *
 * 交替出现的思考与工具会让相邻工具分组几乎永远只凑出一项而退化为不分组，
 * 纵向空间因此失控。按决策-执行单元聚合后，一行摘要能同时表达"想了什么"
 * 与"做了什么"。
 *
 * 正文、权限、提问等部件是模型对用户说话，天然构成步骤边界；运行中或失败的
 * 工具需要保持可见，因此不并入步骤组，回落到既有的相邻工具分组逻辑。
 *
 * @param parts 原始消息部件
 * @returns 保持原顺序的步骤组、工具组与普通部件
 */
export function groupIntoSteps(parts: LiveMessagePart[]): StepMessagePart[] {
  const result: StepMessagePart[] = [];
  // 暂存尚未确定能否成组的部件，遇到边界时按既有规则回落
  let pending: LiveMessagePart[] = [];
  let reasoning: Extract<LiveMessagePart, { type: "reasoning" }> | null = null;
  let tools: Array<{ id: string; tool: ToolLifecycle }> = [];

  /** 将暂存部件按既有分组规则写入结果。 */
  const flushPending = () => {
    if (pending.length > 0) {
      result.push(...groupCompletedToolCalls(pending));
      pending = [];
    }
  };

  /** 结束当前步骤：满足成组条件则合并，否则原样回落。 */
  const flushStep = () => {
    if (!reasoning) return;
    if (tools.length === 0) {
      pending.push(reasoning);
    } else {
      result.push({
        type: "step",
        // 仅用思考部件的 id，组内增长时不重挂载，避免展开状态被重置
        id: `step-${reasoning.id}`,
        reasoning,
        tools: tools.map((item) => item.tool)
      });
    }
    reasoning = null;
    tools = [];
  };

  for (const part of parts) {
    if (part.type === "reasoning") {
      // 思考尚未结束时说明本步骤仍在进行，交回既有逻辑保持实时展示
      if (!part.endedAt) {
        flushStep();
        flushPending();
        result.push({ type: "part", id: part.id, part });
        continue;
      }
      flushStep();
      flushPending();
      reasoning = part;
      continue;
    }
    if (part.type === "tool" && part.tool.status === "completed" && reasoning) {
      tools.push({ id: part.id, tool: part.tool });
      continue;
    }
    // 其余部件都是步骤边界
    flushStep();
    pending.push(part);
  }
  flushStep();
  flushPending();
  return result;
}
