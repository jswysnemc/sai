import { Link } from "react-router-dom";
import type { AgentToolOption } from "./agents-types";
import {
  countToolModes,
  resolveToolMode,
  type ToolMode,
  type ToolModeSelection
} from "./agent-tool-mode-state";
import { ToolModeSwitch } from "./agent-tool-mode-switch";

/** 三段切换的档位文案，由容器统一提供以复用 i18n */
export type ToolModeLabels = {
  value: ToolMode;
  label: string;
  title: string;
}[];

type AgentToolModeGroupProps = {
  /** 分组标识 */
  group: string;
  /** 分组显示名称 */
  label: string;
  /** 用户/模型分工说明；空则不渲染 */
  hint?: string;
  /** 相关设置页路径 */
  settingsHref?: string;
  /** 设置页链接文案 */
  settingsLabel?: string;
  /** 分组内全部工具，用于批量操作 */
  allItems: AgentToolOption[];
  /** 当前搜索命中的工具 */
  visibleItems: AgentToolOption[];
  /** 当前启用与延迟集合 */
  selection: ToolModeSelection;
  /** 三段切换档位文案 */
  modeLabels: ToolModeLabels;
  /** 批量选择的可访问名称模板 */
  groupAriaLabel: string;
  /** 单项选择的可访问名称构造函数 */
  itemAriaLabel: (name: string) => string;
  /** 状态变化回调 */
  onChange: (names: string[], mode: ToolMode) => void;
};

/**
 * 渲染单个用途分组的工具三段权限。
 *
 * 分组头部提供批量切换，逐项行提供独立切换，两者共用同一套档位文案。
 *
 * @param props 分组信息、当前选择与变化回调
 * @returns 分组权限面板
 */
export function AgentToolModeGroup({
  group,
  label,
  hint,
  settingsHref,
  settingsLabel,
  allItems,
  visibleItems,
  selection,
  modeLabels,
  groupAriaLabel,
  itemAriaLabel,
  onChange
}: AgentToolModeGroupProps) {
  const groupNames = allItems.map((tool) => tool.name);
  const isBase = (name: string) => allItems.find((tool) => tool.name === name)?.group === "base";
  const counts = countToolModes(selection, groupNames, isBase);
  // 分组整体处于同一状态时高亮该档位，混合状态下不高亮任何档位
  const uniformMode = (["on", "load", "off"] as ToolMode[])
    .find((mode) => counts[mode] === groupNames.length);

  return (
    <section className="agent-tool-permission-group" data-group={group}>
      <header className="agent-tool-permission-group-head">
        <div className="agent-tool-permission-group-head-row">
          <div className="agent-tool-permission-group-title">
            <strong>{label}</strong>
            {label !== group && <small>{group}</small>}
          </div>
          <ToolModeSwitch
            value={uniformMode ?? ("" as ToolMode)}
            options={modeLabels}
            ariaLabel={groupAriaLabel}
            onChange={(mode) => onChange(groupNames, mode)}
          />
        </div>
        {hint ? <p className="agent-tool-permission-group-hint">{hint}</p> : null}
        {settingsHref ? (
          <Link className="agent-tool-permission-group-link" to={settingsHref}>
            {settingsLabel ?? settingsHref}
          </Link>
        ) : null}
      </header>
      <div className="agent-tool-permission-items">
        {visibleItems.map((tool) => {
          const mode = resolveToolMode(selection, tool.name, tool.group === "base");
          return (
            <div
              key={tool.name}
              className="agent-tool-permission-item"
              data-mode={mode}
              title={tool.description || tool.name}
            >
              <span className="agent-tool-permission-copy">
                <strong>{tool.name}</strong>
                {tool.description && <small>{tool.description}</small>}
              </span>
              <ToolModeSwitch
                value={mode}
                options={modeLabels}
                ariaLabel={itemAriaLabel(tool.name)}
                onChange={(next) => onChange([tool.name], next)}
              />
            </div>
          );
        })}
      </div>
    </section>
  );
}
