import { Link } from "react-router-dom";
import { Cable, Cpu } from "../../shared/ui/icons";
import type { SuperAgentCapabilities } from "../../api/super-agent-contracts";
import { useI18n } from "../i18n/use-i18n";

type CapabilitiesGuideCardProps = {
  capabilities?: SuperAgentCapabilities;
};

/**
 * 【超级智能体】【渠道引导与工具矩阵】展示微信/QQ/飞书网关实时状态、接入步骤，
 * 以及暴露给 Leader(Router) 与 Worker(Runner) 的完整工具边界设计。
 *
 * @param props 后端返回的实时能力与网关状态快照
 * @returns 渠道接入引导与工具设计区块
 */
export function CapabilitiesGuideCard({ capabilities }: CapabilitiesGuideCardProps) {
  const { t } = useI18n();
  const gw = capabilities?.gateways;

  return (
    <>
      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <Cable size={16} />
            {t("Weixin / Multi-Channel Gateway Guide", "微信与多渠道接入指南（实时网关状态）")}
          </span>
          <Link to="/gateways" className="sa-back-link">
            {t("Open Gateways", "前往网关管理 / 微信扫码")}
          </Link>
        </header>

        {gw && (
          <div className="sa-group-meta">
            <span className={gw.weixin.running ? "sa-badge accent" : "sa-badge"}>
              {t("Weixin", "微信 iLink")}:{" "}
              {gw.weixin.running
                ? t("Running", "运行中")
                : gw.weixin.logged_in
                  ? t("Logged in / Stopped", "已登录未启动")
                  : t("Not logged in", "未登录")}
            </span>
            <span className={gw.qq.running ? "sa-badge accent" : "sa-badge"}>
              QQ:{" "}
              {gw.qq.running
                ? t("Running", "运行中")
                : gw.qq.configured
                  ? t("Configured / Stopped", "已配置未启动")
                  : t("Not configured", "未配置")}
            </span>
            <span className={gw.feishu.configured ? "sa-badge accent" : "sa-badge"}>
              {t("Feishu", "飞书")}: {gw.feishu.configured ? t("Configured", "已配置") : t("Not configured", "未配置")}
            </span>
            <span className="sa-badge">
              {gw.channel_leader_routing
                ? t("Auto-route to Leader: ON", "渠道自动接入 Leader：开启")
                : t("Bound chats & /leader only", "仅已绑定聊天与 /leader")}
            </span>
          </div>
        )}

        <div className="sa-grid-2">
          <article className="sa-item-card">
            <strong>{t("1. Login & Start Weixin Gateway", "1. 微信扫码登录并启动网关")}</strong>
            <div style={{ fontSize: "0.78rem", color: "var(--ink-soft)" }}>
              {t(
                "Open Gateways page (or run `sai gateway weixin-login`) to scan QR code, then start the gateway (`sai gateway weixin-server` or `/gateway start weixin` in TUI).",
                "在「网关管理」页点击微信扫码登录（或终端执行 sai gateway weixin-login），随后启动微信网关（WebUI 按钮、终端 sai gateway weixin-server 或群组 TUI 内执行 /gateway start weixin）。"
              )}
            </div>
          </article>
          <article className="sa-item-card">
            <strong>{t("2. Chat or Bind from Weixin / Feishu / QQ", "2. 在微信/飞书/QQ 中直接与 Leader 对话")}</strong>
            <div style={{ fontSize: "0.78rem", color: "var(--ink-soft)" }}>
              {t(
                "With auto-routing enabled, send tasks directly to the bot; or use `/group bind`, `/group new <name>`, `/leader <task>`, and `/runners` inside the IM chat. Leader synthesizes Runner reports and pushes results back.",
                "默认开启自动路由时，直接给机器人发消息即交由群组 Leader 编排；也可在聊天内发 /group bind 绑定群组、/leader <任务> 显式派发、/runners 查看进度，Runner 完成后由 Leader 主动归纳回推。"
              )}
            </div>
          </article>
        </div>
      </section>

      {capabilities && (
        <section className="sa-section-card">
          <header className="sa-section-title">
            <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
              <Cpu size={16} />
              {t("Leader & Runner Tool Architecture", "暴露给 Leader 与 Runner 的工具边界设计")}
            </span>
          </header>

          <div style={{ fontSize: "0.78rem", color: "var(--ink-soft)" }}>
            {t(
              "Leader acts as the lightweight Router & Orchestrator: it permanently loads 9 super_* orchestration tools plus lightweight read-only perception tools, while heavy execution tools are stripped and delegated to isolated Runners.",
              "Leader 作为轻量常驻 Router/编排层，默认常驻加载 9 个 super_* 编排工具及轻量只读感知工具，重执行工具全部隔离剥离并委派给独立上下文的 Runner 执行。"
            )}
          </div>

          <div className="sa-grid-2">
            {capabilities.leader_tools.map((tool) => (
              <article key={tool.name} className="sa-item-card">
                <div className="sa-group-card-top">
                  <strong>{tool.name}</strong>
                  <span className="sa-badge accent">{tool.category}</span>
                </div>
                <div style={{ fontSize: "0.78rem", color: "var(--ink-soft)" }}>{tool.summary}</div>
              </article>
            ))}
          </div>

          <div className="sa-grid-2">
            <article className="sa-item-card">
              <strong>{t("Leader Read-only Perception", "Leader 保留的轻量只读工具")}</strong>
              <div className="sa-group-meta">
                {capabilities.leader_readonly_tools.map((name) => (
                  <span key={name} className="sa-badge">{name}</span>
                ))}
              </div>
            </article>
            <article className="sa-item-card">
              <strong>{t("Excluded from Leader (Delegated to Runners)", "从 Leader 剥离的重执行工具（强制交由 Runner）")}</strong>
              <div className="sa-group-meta">
                {capabilities.leader_excluded_tools.map((name) => (
                  <span key={name} className="sa-badge danger">{name}</span>
                ))}
              </div>
            </article>
          </div>

          <div className="sa-grid-2">
            {capabilities.worker_tools.map((tool) => (
              <article key={tool.name} className="sa-item-card">
                <div className="sa-group-card-top">
                  <strong>{tool.name}</strong>
                  <span className="sa-badge">Runner · {tool.category}</span>
                </div>
                <div style={{ fontSize: "0.78rem", color: "var(--ink-soft)" }}>{tool.summary}</div>
              </article>
            ))}
          </div>
        </section>
      )}
    </>
  );
}
