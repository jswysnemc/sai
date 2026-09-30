import { useI18n } from "../../i18n/use-i18n";
import { MarkdownRenderer } from "../markdown-renderer";
import { DisclosureItem } from "./disclosure-item";
import { ToolPanel } from "./layout/tool-panel";
import type { LoadedSkill, LoadedTool, LoadResult } from "./load-tool-data";
import "./load-tool-view.css";

type LoadToolViewProps = {
  result: LoadResult;
};

/**
 * 【工具结果】【load】展示 load 读取到的 Skill 文档或工具 Schema。
 *
 * 每项默认折叠：Skill 显示名称与说明，展开看 Markdown 正文；
 * 工具显示名称与说明，展开看参数表。已在本会话加载过的项不再返回全文，只标状态。
 *
 * @param props 已解析的 load 结果
 * @returns load 结果视图
 */
export function LoadToolView({ result }: LoadToolViewProps) {
  const { t } = useI18n();
  const empty = result.items.length === 0;
  return (
    <ToolPanel className="load-tool-view">
      <div className="load-tool">
        <h3>{result.kind === "skill" ? "Skills" : t("Tool schemas", "工具 Schema")}</h3>
        {empty ? <p className="load-tool-empty">{t("Nothing was loaded.", "没有加载任何内容。")}</p> : null}
        <ul>
          {result.kind === "skill"
            ? result.items.map((skill) => <SkillItem key={skill.name} skill={skill} />)
            : result.items.map((tool) => <ToolItem key={tool.name} tool={tool} />)}
        </ul>
      </div>
    </ToolPanel>
  );
}

/**
 * 单个 Skill：说明作为副标题，正文按需展开渲染 Markdown。
 *
 * @param props Skill 文档
 * @returns 可展开条目
 */
function SkillItem({ skill }: { skill: LoadedSkill }) {
  const { t } = useI18n();
  const meta = skill.status === "already_loaded"
    ? t("already loaded earlier", "此前已加载")
    : skill.description;
  return (
    <DisclosureItem title={skill.name} meta={meta}>
      {skill.body ? (
        <div className="load-tool-markdown">
          <MarkdownRenderer source={skill.body} />
        </div>
      ) : undefined}
    </DisclosureItem>
  );
}

/**
 * 单个工具：说明作为副标题，展开后列出参数名、类型与说明。
 *
 * @param props 工具 Schema
 * @returns 可展开条目
 */
function ToolItem({ tool }: { tool: LoadedTool }) {
  const { t } = useI18n();
  const meta = tool.status === "already_loaded"
    ? t("already loaded earlier", "此前已加载")
    : firstSentence(tool.description);
  return (
    <DisclosureItem title={tool.name} meta={meta}>
      {tool.parameters.length > 0 ? (
        <table className="load-tool-params">
          <thead>
            <tr>
              <th scope="col">{t("Parameter", "参数")}</th>
              <th scope="col">{t("Type", "类型")}</th>
              <th scope="col">{t("Description", "说明")}</th>
            </tr>
          </thead>
          <tbody>
            {tool.parameters.map((param) => (
              <tr key={param.name}>
                <td>
                  <code>{param.name}</code>
                  {param.required ? <span className="load-tool-required" title={t("required", "必填")}>*</span> : null}
                </td>
                <td><code>{param.type}</code></td>
                <td>{param.description}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : undefined}
    </DisclosureItem>
  );
}

/**
 * 取说明首句，避免副标题铺满整行。
 *
 * @param value 原始说明
 * @returns 首句
 */
function firstSentence(value: string): string {
  const single = value.replace(/\s+/g, " ").trim();
  return single.split(/(?<=[。.!？?])\s/u)[0] ?? single;
}
