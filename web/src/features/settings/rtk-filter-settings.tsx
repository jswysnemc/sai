import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import type { AppConfig } from "../../api/contracts";
import { api } from "../../api/client";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import "./rtk-filter-settings.css";

type RtkFilterSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 rtk 命令输出过滤器的专属配置。
 *
 * 代理范围由 rtk 自身决定：服务端探测它的子命令并逐条询问映射，
 * 能代理的默认全部交给它。界面展示这份能力清单，被排除的命令在其中标灰划掉。
 *
 * @param props 应用配置与更新回调
 * @returns rtk 过滤器配置区域
 */
export function RtkFilterSettings({ config, onConfigChange }: RtkFilterSettingsProps) {
  const { t } = useI18n();
  const status = useQuery({ queryKey: ["rtk-status"], queryFn: api.config.rtkStatus, staleTime: 60_000 });
  const [draftItem, setDraftItem] = useState("");

  const tools = (config.tools as Record<string, unknown> | undefined) ?? {};
  const mode = typeof tools.command_filter === "string" ? tools.command_filter : "auto";
  const denylist = Array.isArray(tools.command_filter_denylist)
    ? (tools.command_filter_denylist as string[])
    : [];
  const proxyCommands = status.data?.proxy_commands ?? [];
  const available = status.data?.available;

  /**
   * 合并补丁并回写工具配置。
   *
   * @param patch 待合并的工具配置片段
   * @returns 无返回值
   */
  const updateTools = (patch: Record<string, unknown>) => {
    onConfigChange({ ...config, tools: { ...tools, ...patch } });
  };

  /**
   * 把输入框内容加入排除列表，支持逗号或空格分隔多项。
   *
   * @returns 无返回值
   */
  const addItems = () => {
    const items = draftItem.split(/[\s,]+/).map((item) => item.trim()).filter(Boolean);
    if (items.length === 0) return;
    const merged = [...denylist];
    for (const item of items) {
      if (!merged.includes(item)) merged.push(item);
    }
    updateTools({ command_filter_denylist: merged });
    setDraftItem("");
  };

  /**
   * 切换某个命令的排除状态。
   *
   * @param name 命令名
   * @returns 无返回值
   */
  const toggleExcluded = (name: string) => {
    const next = denylist.includes(name)
      ? denylist.filter((item) => item !== name)
      : [...denylist, name];
    updateTools({ command_filter_denylist: next });
  };

  // rtk 能力集之外的排除项：用户手动加过、但当前 rtk 版本并不代理的命令
  const extraExcluded = denylist.filter((item) => !proxyCommands.includes(item));

  const modeOptions = [
    {
      value: "auto",
      label: t("Auto", "自动"),
      description: t("Enable when rtk is detected on PATH", "PATH 中检测到 rtk 时启用")
    },
    {
      value: "rtk",
      label: t("Force on", "强制启用"),
      description: t("Always route commands through rtk", "始终经由 rtk 执行命令")
    },
    {
      value: "off",
      label: t("Off", "关闭"),
      description: t("Never rewrite commands", "从不改写命令")
    }
  ];

  return (
    <div className="rtk-filter-settings">
      <div className="rtk-filter-status">
        {status.isLoading ? (
          <span className="rtk-status-badge unknown">{t("Detecting rtk...", "正在探测 rtk...")}</span>
        ) : available ? (
          <span className="rtk-status-badge ok">{t("rtk detected", "已检测到 rtk")}</span>
        ) : (
          <span className="rtk-status-badge missing">{t("rtk not installed", "未检测到 rtk")}</span>
        )}
        {available === false && (
          <span className="rtk-status-note">
            {t(
              "Install rtk and make it available on PATH, otherwise auto/force modes have no effect.",
              "安装 rtk 并确保在 PATH 中可用，否则自动/强制档位不会生效。"
            )}
          </span>
        )}
      </div>
      <div className="settings-field">
        <span>{t("Filter mode", "过滤档位")}</span>
        <Select
          value={mode}
          options={modeOptions}
          onChange={(value) => updateTools({ command_filter: value })}
          ariaLabel={t("Command output filter mode", "命令输出过滤器档位")}
        />
        <small>{t(
          "Commands are rewritten to \"rtk <command>\" to compress output entering the context. Compound commands with pipes or redirects are always left as-is.",
          "命令会被改写为 \"rtk <命令>\"，压缩进入上下文的输出。含管道或重定向的复合命令始终保持原样。"
        )}</small>
      </div>
      <div className="settings-field full">
        <span>
          {t(
            `Proxied commands (${proxyCommands.length - denylist.filter((item) => proxyCommands.includes(item)).length}/${proxyCommands.length})`,
            `已代理的命令（${proxyCommands.length - denylist.filter((item) => proxyCommands.includes(item)).length}/${proxyCommands.length}）`
          )}
        </span>
        {/* 点击标签即可排除或恢复某个命令，被排除的划掉标灰 */}
        <div className="rtk-command-tags">
          {proxyCommands.map((name) => {
            const excluded = denylist.includes(name);
            return (
              <button
                key={name}
                type="button"
                className={excluded ? "rtk-command-tag excluded" : "rtk-command-tag"}
                title={excluded ? t("Click to proxy again", "点击恢复代理") : t("Click to exclude", "点击排除")}
                onClick={() => toggleExcluded(name)}
              >
                {name}
              </button>
            );
          })}
          {proxyCommands.length === 0 && (
            <span className="rtk-denylist-empty">
              {t("Install rtk to see the commands it can proxy.", "安装 rtk 后这里会列出它能代理的命令。")}
            </span>
          )}
        </div>
        <small>
          {t(
            "Every command rtk supports is proxied by default. Click a tag to exclude it. Commands rtk does not support, compound commands with pipes or redirects, and interactive subcommands are never rewritten.",
            "rtk 支持的命令默认全部代理。点击标签可排除某一项。rtk 不支持的命令、含管道或重定向的复合命令、交互式子命令都不会被改写。"
          )}
        </small>
      </div>
      {extraExcluded.length > 0 && (
        <div className="settings-field full">
          <span>{t("Excluded but not proxied by rtk", "已排除但 rtk 并不代理")}</span>
          <div className="rtk-command-tags">
            {extraExcluded.map((name) => (
              <button
                key={name}
                type="button"
                className="rtk-command-tag excluded"
                title={t("Remove from the list", "从列表中移除")}
                onClick={() => toggleExcluded(name)}
              >
                {name}<span aria-hidden="true">×</span>
              </button>
            ))}
          </div>
          <small>
            {t(
              "These are already outside rtk's reach, so the entries have no effect. Click to remove them.",
              "这些命令本就不在 rtk 的代理范围内，条目不起作用。点击可移除。"
            )}
          </small>
        </div>
      )}
      <div className="settings-field full">
        <span>{t("Exclude a command", "排除命令")}</span>
        <div className="rtk-denylist-input">
          <input
            type="text"
            value={draftItem}
            placeholder={t("Command name, e.g. git", "命令名，如 git")}
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => setDraftItem(event.target.value)}
            onKeyDown={(event) => {
              if (event.key !== "Enter") return;
              event.preventDefault();
              addItems();
            }}
          />
          <button type="button" className="settings-secondary" onClick={addItems} disabled={!draftItem.trim()}>
            {t("Add", "添加")}
          </button>
          {denylist.length > 0 && (
            <button
              type="button"
              className="settings-secondary"
              onClick={() => updateTools({ command_filter_denylist: [] })}
            >
              {t("Proxy all again", "全部恢复代理")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
