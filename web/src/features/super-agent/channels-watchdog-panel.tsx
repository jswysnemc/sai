import { useEffect, useState } from "react";
import { Cable, CheckCircle2, RefreshCw, Save, ShieldCheck, Trash2 } from "../../shared/ui/icons";
import type {
  BindChannelPayload,
  SuperAgentConfig,
  SuperChannelBinding,
  SuperDeliveryReceipt,
  SuperWatchdogEvent
} from "../../api/super-agent-contracts";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";

type ChannelsWatchdogPanelProps = {
  channels: SuperChannelBinding[];
  deliveries: SuperDeliveryReceipt[];
  watchdogEvents: SuperWatchdogEvent[];
  config?: SuperAgentConfig;
  busy: boolean;
  onBindChannel: (payload: BindChannelPayload) => Promise<void>;
  onUnbindChannel: (bindingId: string) => Promise<void>;
  onRunWatchdog: () => Promise<void>;
  onSaveConfig: (next: SuperAgentConfig) => Promise<void>;
};

/**
 * 【超级智能体】【渠道与守护控制面板】管理外部消息渠道绑定、Leader 成果回推回执、Watchdog 巡检与双层模型运行配置。
 *
 * @param props 渠道绑定、回执、守护事件与配置数据
 * @returns 渠道与 Watchdog 管理视图
 */
export function ChannelsWatchdogPanel({
  channels,
  deliveries,
  watchdogEvents,
  config,
  busy,
  onBindChannel,
  onUnbindChannel,
  onRunWatchdog,
  onSaveConfig
}: ChannelsWatchdogPanelProps) {
  const { t } = useI18n();
  const [channel, setChannel] = useState("weixin");
  const [chatId, setChatId] = useState("*");
  const [label, setLabel] = useState("");
  const [admission, setAdmission] = useState("auto");
  const [draftCfg, setDraftCfg] = useState<SuperAgentConfig | null>(config ?? null);

  useEffect(() => {
    if (config) {
      setDraftCfg(config);
    }
  }, [config]);

  const handleBind = async () => {
    await onBindChannel({
      channel,
      external_chat_id: chatId.trim() || "*",
      label: label.trim() || `${channel}:${chatId.trim() || "*"}`,
      admission_mode: admission,
      enabled: true
    });
    setLabel("");
  };

  return (
    <div className="sa-panel-scroll">
      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <Cable size={16} />
            {t("Bound Channels (Channel -> Leader -> Runner -> Delivery)", "渠道绑定（渠道直连 Leader 编排与回推）")} ({channels.length})
          </span>
        </header>

        <div className="sa-form-grid">
          <label className="sa-field">
            <span>{t("Channel", "渠道类型")}</span>
            <select className="sa-select" value={channel} onChange={(e) => setChannel(e.target.value)}>
              <option value="weixin">Weixin</option>
              <option value="feishu">Feishu</option>
              <option value="qq">QQ</option>
              <option value="onebot">OneBot</option>
            </select>
          </label>
          <label className="sa-field">
            <span>{t("External Chat ID (* for all)", "外部聊天 ID（* 匹配全部）")}</span>
            <input className="sa-input" value={chatId} onChange={(e) => setChatId(e.target.value)} />
          </label>
          <label className="sa-field">
            <span>{t("Label", "备注名称")}</span>
            <input
              className="sa-input"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder={t("e.g. Engineering Feishu", "例如：研发飞书群")}
            />
          </label>
          <label className="sa-field">
            <span>{t("Admission policy", "准入策略")}</span>
            <select className="sa-select" value={admission} onChange={(e) => setAdmission(e.target.value)}>
              <option value="auto">auto</option>
              <option value="mentions_only">mentions_only</option>
              <option value="manual">manual</option>
            </select>
          </label>
          <Button variant="primary" size="small" disabled={busy} onClick={() => void handleBind()}>
            {t("Bind channel", "绑定渠道")}
          </Button>
        </div>

        {channels.length === 0 ? (
          <div className="sa-empty">
            {t("No external channels bound to this group yet.", "当前群组尚未绑定外部渠道。")}
          </div>
        ) : (
          <div className="sa-grid-2">
            {channels.map((item) => (
              <article key={item.id} className="sa-item-card">
                <div className="sa-group-card-top">
                  <strong>{item.label || `${item.channel}:${item.external_chat_id}`}</strong>
                  <Button
                    size="icon"
                    variant="ghost-danger"
                    aria-label={t("Unbind channel", "解绑渠道")}
                    disabled={busy}
                    onClick={() => void onUnbindChannel(item.id)}
                  >
                    <Trash2 size={14} />
                  </Button>
                </div>
                <div className="sa-group-meta">
                  <span className="sa-badge accent">{item.channel}</span>
                  <span>chat: {item.external_chat_id}</span>
                  <span>mode: {item.admission_mode}</span>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>

      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <CheckCircle2 size={16} />
            {t("Leader Delivery Receipts", "Leader 成果回推回执")} ({deliveries.length})
          </span>
        </header>
        {deliveries.length === 0 ? (
          <div className="sa-empty">{t("No deliveries recorded yet.", "暂无成果回推记录。")}</div>
        ) : (
          <div className="sa-grid-2">
            {deliveries.slice(0, 12).map((d) => (
              <article key={d.id} className="sa-item-card">
                <div className="sa-group-card-top">
                  <strong>{d.target}</strong>
                  <span className={d.status === "failed" ? "sa-badge danger" : "sa-badge accent"}>{d.status}</span>
                </div>
                <div style={{ fontSize: "0.78rem" }}>{d.preview}</div>
                {d.error && <div className="sa-inline-error">{d.error}</div>}
              </article>
            ))}
          </div>
        )}
      </section>

      <section className="sa-section-card">
        <header className="sa-section-title">
          <span style={{ display: "inline-flex", alignItems: "center", gap: "0.4rem" }}>
            <ShieldCheck size={16} />
            {t("Watchdog Control Loop", "外部守护回路（Watchdog）")} ({watchdogEvents.length})
          </span>
          <Button size="small" variant="secondary" disabled={busy} onClick={() => void onRunWatchdog()}>
            <RefreshCw size={14} />
            {t("Run check", "立即巡检")}
          </Button>
        </header>
        {watchdogEvents.length === 0 ? (
          <div className="sa-empty">{t("No watchdog events yet.", "暂无巡检记录。")}</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.4rem" }}>
            {watchdogEvents.slice(0, 10).map((ev) => (
              <div key={ev.id} className="sa-item-card">
                <div className="sa-group-card-top">
                  <span className={ev.severity === "action_required" ? "sa-badge danger" : "sa-badge accent"}>
                    {ev.severity}
                  </span>
                  <span style={{ fontSize: "0.72rem", color: "var(--ink-soft)" }}>
                    {new Date(ev.created_at).toLocaleTimeString()}
                  </span>
                </div>
                <div style={{ fontSize: "0.8rem" }}>{ev.summary}</div>
                {ev.details && <div className="sa-group-meta">{ev.details}</div>}
              </div>
            ))}
          </div>
        )}
      </section>

      {draftCfg && (
        <section className="sa-section-card">
          <header className="sa-section-title">
            <span>{t("Super-Agent Architecture Settings", "Super-Agent 双层架构配置")}</span>
            <Button size="small" variant="primary" disabled={busy} onClick={() => void onSaveConfig(draftCfg)}>
              <Save size={14} />
              {t("Save settings", "保存配置")}
            </Button>
          </header>
          <div className="sa-form-grid">
            <label className="sa-field">
              <span>{t("Channel -> Leader auto routing", "网关渠道自动接入 Leader")}</span>
              <select
                className="sa-select"
                value={draftCfg.channel_leader_routing ? "on" : "off"}
                onChange={(e) => setDraftCfg({ ...draftCfg, channel_leader_routing: e.target.value === "on" })}
              >
                <option value="on">{t("Enabled", "开启")}</option>
                <option value="off">{t("Bound chats & /leader only", "仅已绑定聊天与 /leader")}</option>
              </select>
            </label>
            <label className="sa-field">
              <span>{t("Max concurrent Runners", "最大并发 Runner 数")}</span>
              <input
                type="number"
                min={1}
                max={32}
                className="sa-input"
                value={draftCfg.max_concurrent_workers}
                onChange={(e) => setDraftCfg({ ...draftCfg, max_concurrent_workers: Number(e.target.value) || 1 })}
              />
            </label>
            <label className="sa-field">
              <span>{t("IM reply word budget", "IM 渠道回复字数上限")}</span>
              <input
                type="number"
                min={40}
                max={1000}
                className="sa-input"
                value={draftCfg.im_reply_max_words}
                onChange={(e) => setDraftCfg({ ...draftCfg, im_reply_max_words: Number(e.target.value) || 180 })}
              />
            </label>
            <label className="sa-field">
              <span>{t("Leader model override", "Leader 独立模型（留空跟随默认）")}</span>
              <input
                className="sa-input"
                value={draftCfg.leader_model}
                onChange={(e) => setDraftCfg({ ...draftCfg, leader_model: e.target.value })}
              />
            </label>
            <label className="sa-field">
              <span>{t("Worker model override", "Worker 独立模型（留空跟随默认）")}</span>
              <input
                className="sa-input"
                value={draftCfg.worker_model}
                onChange={(e) => setDraftCfg({ ...draftCfg, worker_model: e.target.value })}
              />
            </label>
            <label className="sa-field">
              <span>{t("Git worktree isolation", "Runner Git Worktree 隔离")}</span>
              <select
                className="sa-select"
                value={draftCfg.worker_use_worktree ? "on" : "off"}
                onChange={(e) => setDraftCfg({ ...draftCfg, worker_use_worktree: e.target.value === "on" })}
              >
                <option value="on">{t("Enabled", "开启")}</option>
                <option value="off">{t("Disabled", "关闭")}</option>
              </select>
            </label>
          </div>
        </section>
      )}
    </div>
  );
}
