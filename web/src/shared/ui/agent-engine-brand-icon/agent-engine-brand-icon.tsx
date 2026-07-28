import claudeCodeIconUrl from "@lobehub/icons-static-svg/icons/claudecode-color.svg";
import codexIconUrl from "@lobehub/icons-static-svg/icons/codex-color.svg";
import { Cpu } from "lucide-react";
import type { AgentEngineKind } from "../../../api/contracts";
import "./agent-engine-brand-icon.css";

type AgentEngineBrandIconProps = {
  engine: AgentEngineKind;
  size?: number;
  className?: string;
};

const BRAND_ASSETS: Partial<Record<AgentEngineKind, { name: string; source: string }>> = {
  claude_code: { name: "claude-code", source: claudeCodeIconUrl },
  codex: { name: "codex", source: codexIconUrl }
};

/**
 * 渲染对话内核对应的品牌图标，未配置品牌资源时使用通用内核图标。
 *
 * @param props 内核标识、图标尺寸和附加样式类
 * @returns 品牌图像或通用回退图标
 */
export function AgentEngineBrandIcon({
  engine,
  size = 16,
  className = ""
}: AgentEngineBrandIconProps) {
  const brand = BRAND_ASSETS[engine];
  const classes = `agent-engine-brand-icon${className ? ` ${className}` : ""}`;

  if (!brand) {
    return (
      <Cpu
        className={classes}
        data-agent-engine-brand="generic"
        size={size}
        aria-hidden="true"
      />
    );
  }

  return (
    <img
      className={classes}
      data-agent-engine-brand={brand.name}
      src={brand.source}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
    />
  );
}
