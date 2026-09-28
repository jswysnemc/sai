import type { EngineStatusResponse } from "../../../api/contracts";
import { SettingsField, SkSelect, SkTextInput } from "../kit";
import { useI18n } from "../../i18n/use-i18n";

type AcpConnectionFieldsProps = {
  acp: Record<string, unknown>;
  runtime: EngineStatusResponse["acp_runtime"];
  onChange: (patch: Record<string, unknown>) => void;
};

/**
 * 渲染外部内核的连接类配置。
 *
 * 只保留装机后基本不动的项：认证方式来自握手响应，与建立连接直接相关。
 * 模型、思考等级、权限模式以及 agent 自报的其余运行参数属于高频调整，
 * 已移到输入区，避免设置页被 agent 上报的配置项撑爆。
 *
 * @param props ACP 配置、运行状态与更新回调
 * @returns 连接配置控件
 */
export function AcpConnectionFields({ acp, runtime, onChange }: AcpConnectionFieldsProps) {
  const { t } = useI18n();
  const authMethods = parseAuthMethods(runtime?.auth_methods);
  const value = typeof acp.auth_method === "string" ? acp.auth_method : "";

  return (
    <SettingsField label={t("ACP authentication method", "ACP 认证方式")} configKey="agent.acp.auth_method" anchor="runtime.agent.acp.auth_method" hint={t("Authentication methods are reported during the agent handshake.", "认证方式由内核在握手时公布。")}>

      {authMethods.length > 0 ? (
        <SkSelect
          value={value}
          options={[{ value: "", label: t("Not configured", "未配置") }, ...authMethods]}
          onChange={(next) => onChange({ auth_method: next })}
          ariaLabel={t("ACP authentication method", "ACP 认证方式")}
        />
      ) : (
        <SkTextInput
          value={value}
          onChange={(value) => onChange({ auth_method: value })}
        />
      )}
    </SettingsField>
  );
}

/**
 * 解析 initialize 响应中的认证方式。
 *
 * @param input agent 公布的认证方式
 * @returns 统一下拉框选项
 */
function parseAuthMethods(input: unknown): Array<{ value: string; label: string; description?: string }> {
  if (!Array.isArray(input)) return [];
  return input.flatMap((candidate) => {
    if (!candidate || typeof candidate !== "object") return [];
    const method = candidate as Record<string, unknown>;
    if (typeof method.id !== "string" || typeof method.name !== "string") return [];
    return [{
      value: method.id,
      label: method.name,
      ...(typeof method.description === "string" ? { description: method.description } : {})
    }];
  });
}
