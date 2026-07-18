import { Plus, Terminal, Trash2, Webhook } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { AppConfig, HookHttpRequest, HookItem } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import { EditorHeader, SettingsGroup } from "./editor-layout";
import { ObjectListPanel } from "./object-list-panel";
import { KeyValueEditor } from "./key-value-editor";

type HooksSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

const HOOK_EVENTS = [
  { value: "agent_start", labelEn: "Agent start", labelZh: "Agent 开始" },
  { value: "agent_end", labelEn: "Agent end", labelZh: "Agent 结束" },
  { value: "turn_start", labelEn: "Turn start", labelZh: "轮次开始" },
  { value: "turn_end", labelEn: "Turn end", labelZh: "轮次结束" },
  { value: "message_start", labelEn: "Message start", labelZh: "消息开始" },
  { value: "message_end", labelEn: "Message end", labelZh: "消息结束" },
  { value: "tool_execution_start", labelEn: "Tool start", labelZh: "工具开始" },
  { value: "tool_execution_end", labelEn: "Tool end", labelZh: "工具结束" }
] as const;

const HTTP_METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE"] as const;

/**
 * 渲染生命周期 Hooks 列表与详情编辑。
 *
 * @param props 应用配置和更新回调
 * @returns Hooks 配置区域
 */
export function HooksSettingsSection({ config, onConfigChange }: HooksSettingsSectionProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const hooks = config.hooks ?? { enabled: true, items: [] };
  const items = hooks.items ?? [];
  const [selectedKey, setSelectedKey] = useState(itemKey(items[0], 0));

  useEffect(() => {
    if (!items.some((item, index) => itemKey(item, index) === selectedKey)) {
      setSelectedKey(itemKey(items[0], 0));
    }
  }, [items, selectedKey]);

  const selectedIndex = Math.max(0, items.findIndex((item, index) => itemKey(item, index) === selectedKey));
  const hook = items[selectedIndex];

  const setHooks = (patch: Partial<NonNullable<AppConfig["hooks"]>>) => {
    onConfigChange({ ...config, hooks: { ...hooks, ...patch } });
  };

  const updateHook = (index: number, patch: Partial<HookItem>) => {
    const next = items.map((item, i) => (i === index ? { ...item, ...patch } : item));
    setHooks({ items: next });
    if (index === selectedIndex && patch.name !== undefined) {
      setSelectedKey(itemKey({ ...items[index], ...patch }, index));
    }
  };

  const addHook = () => {
    const name = uniqueHookName(items);
    const next: HookItem = {
      name,
      enabled: true,
      event: "agent_end",
      kind: "command",
      script: 'echo "$SAI_HOOK_EVENT $SAI_SESSION_ID"',
      timeout_ms: 30_000,
      requests: []
    };
    setHooks({ items: [...items, next] });
    setSelectedKey(itemKey(next, items.length));
  };

  const deleteHook = async () => {
    if (!hook) return;
    const confirmed = await confirm({
      title: t("Delete hook", "删除 Hook"),
      description: t(`Delete “${hook.name}”? Failures elsewhere are unaffected.`, `删除“${hook.name}”？其它配置不受影响。`),
      confirmLabel: t("Delete hook", "删除 Hook"),
      danger: true
    });
    if (!confirmed) return;
    const next = items.filter((_, index) => index !== selectedIndex);
    setHooks({ items: next });
    setSelectedKey(itemKey(next[0], 0));
  };

  const eventOptions = useMemo(
    () => HOOK_EVENTS.map((event) => ({ value: event.value, label: t(event.labelEn, event.labelZh) })),
    [t]
  );

  if (!hook) {
    return (
      <div className="settings-objects-layout">
        <ObjectListPanel
          title="Hooks"
          items={[]}
          selectedId=""
          searchPlaceholder={t("Search hooks", "搜索 Hook")}
          addLabel={t("Add hook", "添加 Hook")}
          onSelect={() => undefined}
          onAdd={addHook}
          headerSlot={
            <label className="settings-toggle-field object-list-toggle">
              <span>
                <strong>{t("Enable hooks", "启用 Hooks")}</strong>
                <small>{t("Master switch for all lifecycle hooks", "全部生命周期钩子总开关")}</small>
              </span>
              <input
                type="checkbox"
                checked={hooks.enabled !== false}
                onChange={(event) => setHooks({ enabled: event.target.checked })}
              />
            </label>
          }
        />
        <section className="settings-editor">
          <div className="settings-empty">
            <p>{t("No hooks yet. Add a command or HTTP action that runs at agent, turn, or tool boundaries.", "还没有 Hook。可在 Agent、轮次或工具边界触发命令或 HTTP。")}</p>
            <button type="button" className="settings-secondary" onClick={addHook}>
              <Plus size={14} />{t("Add hook", "添加 Hook")}
            </button>
          </div>
        </section>
      </div>
    );
  }

  const kind = hook.kind ?? "command";
  const request: HookHttpRequest = hook.requests?.[0] ?? { id: "1", url: "", method: "POST", headers: {}, body: "" };
  const methodOptions = HTTP_METHODS.map((method) => ({ value: method, label: method }));

  return (
    <div className="settings-objects-layout">
      <ObjectListPanel
        title="Hooks"
        items={items.map((item, index) => ({
          id: itemKey(item, index),
          name: item.name || t("Untitled hook", "未命名 Hook"),
          meta: `${eventLabel(item.event, t)} · ${item.kind === "http" ? "HTTP" : "Shell"}`,
          icon: item.kind === "http" ? <Webhook size={14} /> : <Terminal size={14} />,
          marked: item.enabled !== false
        }))}
        selectedId={selectedKey}
        searchPlaceholder={t("Search hooks", "搜索 Hook")}
        addLabel={t("Add hook", "添加 Hook")}
        onSelect={setSelectedKey}
        onAdd={addHook}
        headerSlot={
          <label className="settings-toggle-field object-list-toggle">
            <span>
              <strong>{t("Enable hooks", "启用 Hooks")}</strong>
              <small>{t("Master switch for all lifecycle hooks", "全部生命周期钩子总开关")}</small>
            </span>
            <input
              type="checkbox"
              checked={hooks.enabled !== false}
              onChange={(event) => setHooks({ enabled: event.target.checked })}
            />
          </label>
        }
      />
      <section className="settings-editor">
        <EditorHeader
          kicker="Hooks"
          title={hook.name || t("Untitled hook", "未命名 Hook")}
          description={t(
            "Run shell scripts or HTTP requests during the conversation lifecycle. Failures are logged and never block the main run.",
            "在对话生命周期执行 shell 或 HTTP。失败只记日志，不阻断主流程。"
          )}
          actions={
            <>
              <label className="settings-switch">
                <input
                  type="checkbox"
                  checked={hook.enabled !== false}
                  onChange={(event) => updateHook(selectedIndex, { enabled: event.target.checked })}
                />
                <span />
                <strong>{hook.enabled !== false ? t("Enabled", "已启用") : t("Disabled", "已禁用")}</strong>
              </label>
              <button type="button" className="settings-danger" onClick={() => void deleteHook()}>
                <Trash2 size={14} />{t("Delete", "删除")}
              </button>
            </>
          }
        />

        <SettingsGroup
          title={t("Basics", "基础")}
          description={t("Name, trigger event, and action type", "名称、触发事件与动作类型")}
        >
          <div className="settings-form-grid">
            <label className="settings-field">
              <span>{t("Name", "名称")}</span>
              <input value={hook.name} onChange={(event) => updateHook(selectedIndex, { name: event.target.value })} spellCheck={false} />
              <small>{t("Stable label for logs and debugging", "日志与排查用的标识")}</small>
            </label>
            <div className="settings-field">
              <span>{t("Event", "事件")}</span>
              <Select
                value={hook.event}
                options={eventOptions}
                onChange={(value) => updateHook(selectedIndex, { event: value })}
                ariaLabel={t("Hook event", "Hook 事件")}
              />
              <small>{t("When this hook should run", "何时触发")}</small>
            </div>
            <div className="settings-field">
              <span>{t("Action type", "动作类型")}</span>
              <Select
                value={kind}
                options={[
                  { value: "command", label: t("Shell command", "Shell 命令") },
                  { value: "http", label: t("HTTP request", "HTTP 请求") }
                ]}
                onChange={(value) => updateHook(selectedIndex, {
                  kind: value,
                  requests: value === "http" && !(hook.requests?.length)
                    ? [{ id: "1", url: "", method: "POST", headers: {}, body: "" }]
                    : hook.requests
                })}
                ariaLabel={t("Hook type", "Hook 类型")}
              />
              <small>{t("Shell runs with sh -lc; HTTP posts event context as JSON by default", "Shell 走 sh -lc；HTTP 默认提交事件 JSON")}</small>
            </div>
            <label className="settings-field">
              <span>{t("Timeout (ms)", "超时（毫秒）")}</span>
              <input
                type="number"
                min={100}
                max={120000}
                value={hook.timeout_ms ?? 30_000}
                onChange={(event) => updateHook(selectedIndex, {
                  timeout_ms: event.target.value === "" ? null : Number(event.target.value)
                })}
              />
              <small>{t("100–120000 ms; default 30000", "范围 100–120000，默认 30000")}</small>
            </label>
          </div>
        </SettingsGroup>

        {kind === "command" ? (
          <SettingsGroup
            title={t("Shell action", "Shell 动作")}
            description={t("Environment variables available to the script", "脚本可用环境变量")}
          >
            <div className="settings-form-grid">
              <label className="settings-field full">
                <span>{t("Script", "脚本")}</span>
                <textarea
                  rows={8}
                  value={hook.script ?? ""}
                  onChange={(event) => updateHook(selectedIndex, { script: event.target.value })}
                  spellCheck={false}
                  placeholder={'echo "$SAI_HOOK_EVENT session=$SAI_SESSION_ID"'}
                />
                <small>
                  {t(
                    "Available: SAI_HOOK_EVENT, SAI_HOOK_NAME, SAI_SESSION_ID, SAI_WORKDIR, SAI_TOOL_NAME (tool events)",
                    "可用：SAI_HOOK_EVENT、SAI_HOOK_NAME、SAI_SESSION_ID、SAI_WORKDIR、SAI_TOOL_NAME（工具事件）"
                  )}
                </small>
              </label>
            </div>
            <div className="settings-hint-grid">
              {[
                ["SAI_HOOK_EVENT", t("Current event name", "当前事件名")],
                ["SAI_SESSION_ID", t("Active session id", "当前会话 ID")],
                ["SAI_WORKDIR", t("Workspace directory", "工作区目录")],
                ["SAI_TOOL_NAME", t("Tool name when applicable", "适用时的工具名")]
              ].map(([key, desc]) => (
                <div className="settings-hint-chip" key={key}>
                  <code>{key}</code>
                  <span>{desc}</span>
                </div>
              ))}
            </div>
          </SettingsGroup>
        ) : (
          <SettingsGroup
            title={t("HTTP action", "HTTP 动作")}
            description={t("Request sent when the selected event fires", "选中事件触发时发出的请求")}
          >
            <div className="settings-form-grid">
              <label className="settings-field full">
                <span>URL</span>
                <input
                  value={request.url}
                  onChange={(event) => updateHook(selectedIndex, {
                    requests: [{ ...request, id: request.id || "1", url: event.target.value }]
                  })}
                  spellCheck={false}
                  placeholder="https://example.com/hooks/sai"
                />
                <small>{t("Target endpoint; event context is sent as JSON when body is empty", "目标地址；body 为空时发送事件 JSON")}</small>
              </label>
              <div className="settings-field">
                <span>{t("Method", "方法")}</span>
                <Select
                  value={(request.method ?? "POST").toUpperCase()}
                  options={methodOptions}
                  onChange={(value) => updateHook(selectedIndex, {
                    requests: [{ ...request, id: request.id || "1", method: value }]
                  })}
                  ariaLabel={t("HTTP method", "HTTP 方法")}
                />
              </div>
              <label className="settings-field full">
                <span>{t("Body template", "请求体模板")}</span>
                <textarea
                  rows={6}
                  value={request.body ?? ""}
                  onChange={(event) => updateHook(selectedIndex, {
                    requests: [{ ...request, id: request.id || "1", body: event.target.value }]
                  })}
                  spellCheck={false}
                  placeholder={t("Leave empty to send the default event JSON", "留空则发送默认事件 JSON")}
                />
                <small>{t("Optional raw body string; empty body uses the built-in event payload", "可选原始 body；留空使用内置事件载荷")}</small>
              </label>
              <div className="settings-field full">
                <span>{t("Headers", "请求头")}</span>
                <KeyValueEditor
                  value={request.headers ?? {}}
                  keyPlaceholder={t("Header name", "Header 名")}
                  valuePlaceholder={t("Header value", "Header 值")}
                  addLabel={t("Add header", "添加请求头")}
                  onChange={(headers) => updateHook(selectedIndex, {
                    requests: [{ ...request, id: request.id || "1", headers }]
                  })}
                />
                <small>{t("Common example: Authorization, Content-Type", "常见：Authorization、Content-Type")}</small>
              </div>
            </div>
          </SettingsGroup>
        )}
      </section>
    </div>
  );
}

/** 列表项稳定键：名称可能重复，所以带上序号。 */
function itemKey(item: HookItem | undefined, index: number): string {
  if (!item) return "";
  return `${index}:${item.name}`;
}

function uniqueHookName(items: HookItem[]): string {
  let suffix = items.length + 1;
  let name = `hook-${suffix}`;
  while (items.some((item) => item.name === name)) {
    suffix += 1;
    name = `hook-${suffix}`;
  }
  return name;
}

function eventLabel(event: string, t: (en: string, zh: string) => string): string {
  const found = HOOK_EVENTS.find((item) => item.value === event);
  return found ? t(found.labelEn, found.labelZh) : event;
}
