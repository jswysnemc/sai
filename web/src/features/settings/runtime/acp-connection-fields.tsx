import type { EngineStatusResponse } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
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
    <div className="settings-field">
      <span>{t("ACP authentication method", "ACP 认证方式")}</span>
      {authMethods.length > 0 ? (
        <Select
          value={value}
          options={[{ value: "", label: t("Not configured", "未配置") }, ...authMethods]}
          onChange={(next) => onChange({ auth_method: next })}
          ariaLabel={t("ACP authentication method", "ACP 认证方式")}
        />
      ) : (
        <input
          type="text"
          value={value}
          onChange={(event) => onChange({ auth_method: event.target.value })}
        />
      )}
      <small>
        {t(
          "Reported by the agent during the handshake. Model, thinking level, and other runtime options are adjusted from the composer.",
          "由内核在握手时公布。模型、思考等级等运行参数在输入区调整。"
        )}
      </small>
    </div>
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
