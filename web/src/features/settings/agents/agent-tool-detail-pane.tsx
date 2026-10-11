import { useI18n } from "../../i18n/use-i18n";
import type { AgentToolOption } from "./agents-types";
import type { ToolMode } from "./agent-tool-mode-state";
import { ToolModeSwitch } from "./agent-tool-mode-switch";
import type { ToolModeLabels } from "./agent-tool-mode-group";
import "./agent-tool-detail-pane.css";

type AgentToolDetailPaneProps = {
  /** 当前选中的工具；为空时展示空态引导 */
  tool: AgentToolOption | null;
  /** 当前工具在所选 Agent 下的权限模式 */
  mode?: ToolMode;
  /** 三段权限档位标签 */
  modeLabels: ToolModeLabels;
  /** 权限模式变更回调 */
  onModeChange: (name: string, mode: ToolMode) => void;
};

type SchemaProperty = {
  name: string;
  type?: string;
  description?: string;
  required?: boolean;
};

/**
 * 【Agent设置】【工具详情】展示选中工具的参数定义、完整说明与当前权限配置。
 *
 * 由权限面板以弹窗打开，用于查阅 Schema、用途与当前权限。
 *
 * @param props 工具对象、权限模式与状态切换回调
 * @returns 工具详情视图或未选中提示
 */
export function AgentToolDetailPane({
  tool,
  mode,
  modeLabels,
  onModeChange
}: AgentToolDetailPaneProps) {
  const { t } = useI18n();

  if (!tool) {
    return (
      <div className="agent-tool-detail-pane is-empty">
        <p className="agent-tool-detail-empty">
          {t("Select a tool on the left to view its schema, description, and permissions.", "点击左侧工具可在此查看其详细参数定义、说明与权限。")}
        </p>
      </div>
    );
  }

  // 1. 【工具详情】【参数解析】从 parameters Schema 提取字段属性清单
  const properties: SchemaProperty[] = [];
  if (tool.parameters && typeof tool.parameters === "object") {
    const rawProps = tool.parameters.properties;
    const requiredList = Array.isArray(tool.parameters.required)
      ? (tool.parameters.required as string[])
      : [];

    if (rawProps && typeof rawProps === "object") {
      for (const [key, value] of Object.entries(rawProps as Record<string, Record<string, unknown>>)) {
        properties.push({
          name: key,
          type: typeof value?.type === "string" ? value.type : undefined,
          description: typeof value?.description === "string" ? value.description : undefined,
          required: requiredList.includes(key)
        });
      }
    }
  }

  const groupText = t(tool.group_label_en || tool.group, tool.group_label || tool.group);
  const residentText = tool.resident
    ? t("Resident", "常驻工具")
    : t("On demand", "按需暴露");

  return (
    <div className="agent-tool-detail-pane" data-tool={tool.name}>
      <header className="agent-tool-detail-head">
        <div className="agent-tool-detail-head-top">
          <div className="agent-tool-detail-headline">
            <h3 className="agent-tool-detail-name">{tool.name}</h3>
            <span className="agent-tool-detail-group-badge">
              {groupText} · {residentText}
            </span>
          </div>
          {mode ? (
            <ToolModeSwitch
              value={mode}
              options={modeLabels}
              ariaLabel={t(`Permission for ${tool.name}`, `设置 ${tool.name} 的权限`)}
              onChange={(next) => onModeChange(tool.name, next)}
            />
          ) : null}
        </div>
        <p className="agent-tool-detail-desc">
          {tool.full_description || tool.description || t("No description.", "暂无说明。")}
        </p>
      </header>

      <div className="agent-tool-detail-body">
        <section className="agent-tool-detail-section">
          <h4 className="agent-tool-detail-section-title">
            {t("Parameters Schema", "参数定义 (Schema)")}
          </h4>
          {properties.length === 0 ? (
            <p className="agent-tool-detail-no-params">
              {t("This tool accepts no arguments.", "该工具无必填或可选参数。")}
            </p>
          ) : (
            <dl className="agent-tool-detail-params">
              {properties.map((prop) => (
                <div key={prop.name} className="agent-tool-detail-param-item">
                  <dt className="agent-tool-detail-param-head">
                    <code className="agent-tool-detail-param-name">{prop.name}</code>
                    {prop.type ? (
                      <span className="agent-tool-detail-param-type">{prop.type}</span>
                    ) : null}
                    {prop.required ? (
                      <span className="agent-tool-detail-param-required">
                        {t("required", "必填")}
                      </span>
                    ) : (
                      <span className="agent-tool-detail-param-optional">
                        {t("optional", "可选")}
                      </span>
                    )}
                  </dt>
                  {prop.description ? (
                    <dd className="agent-tool-detail-param-desc">{prop.description}</dd>
                  ) : null}
                </div>
              ))}
            </dl>
          )}
        </section>
      </div>
    </div>
  );
}
