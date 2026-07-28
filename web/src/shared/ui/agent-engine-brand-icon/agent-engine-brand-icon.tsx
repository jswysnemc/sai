import claudeCodeIconUrl from "@lobehub/icons-static-svg/icons/claudecode-color.svg";
import codexIconUrl from "@lobehub/icons-static-svg/icons/codex-color.svg";
import { Cpu } from "lucide-react";
import "./agent-engine-brand-icon.css";

type AgentEngineBrandIconProps = {
  engine: string;
  size?: number;
  className?: string;
};

type BrandedEngineKind = "claude_code" | "codex";

const BRAND_ASSETS: Record<BrandedEngineKind, { name: string; source: string }> = {
  claude_code: { name: "claude-code", source: claudeCodeIconUrl },
  codex: { name: "codex", source: codexIconUrl }
};

/**
 * 从配置标识或 ACP 握手展示名称解析品牌资源。
 *
 * @param engine 内核配置标识或展示名称
 * @returns 已知品牌资源；未知内核返回空值
 */
function resolveBrandAsset(engine: string): { name: string; source: string } | undefined {
  const normalized = engine.trim().toLowerCase().replace(/[\s-]+/g, "_");
  if (normalized !== "claude_code" && normalized !== "codex") return undefined;
  return BRAND_ASSETS[normalized];
}

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
  const brand = resolveBrandAsset(engine);
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
