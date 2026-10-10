import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { useI18n } from "../../i18n/use-i18n";
import { parseSkillDocument } from "../../settings/skills/skill-document";
import type { JevExposedResource } from "./jev-capability-data";

// ===== 新增 import =====
import { exposureBodyDescription } from "./jev-capability-data";

/**
 * 【Jev】【资源详情】工具展开后显示尚未在折叠行出现的说明，以及暴露的参数表。
 *
 * 参数表是 Jev 暴露给模型的 schema，不是这次调用传入的参数。
 *
 * @param props 工具条目，以及折叠行已经展示过的说明
 * @returns 工具详情；既无说明也无参数时为提示文字
 */
export function JevToolDetail({ item, shownMeta = "" }: { item: JevExposedResource; shownMeta?: string }) {
  const { locale, t } = useI18n();
  const parameters = item.parameters ?? [];
  const description = exposureBodyDescription(item, locale, shownMeta);
  if (!description && parameters.length === 0) {
    return <p className="jev-resource-empty">{t("No description or parameters.", "没有说明或参数。")}</p>;
  }
  return (
    <div className="jev-resource-detail">
      {description ? <p className="jev-resource-description">{description}</p> : null}
      {parameters.length > 0 ? (
        <>
          <p className="jev-resource-schema-caption">{t("Exposed schema, not this call's arguments", "暴露的参数，不是本次调用参数")}</p>
          <dl className="jev-resource-params" aria-label={t("Exposed schema", "暴露的参数")}>
            {parameters.map((parameter) => (
              <div key={parameter.name} className="jev-resource-param">
                <dt>
                  <code>{parameter.name}</code>
                  {parameter.type ? <span className="jev-resource-param-type">{parameter.type}</span> : null}
                  {parameter.required ? <span className="jev-resource-param-required">{t("required", "必填")}</span> : null}
                </dt>
                {parameter.description ? <dd>{parameter.description}</dd> : null}
              </div>
            ))}
          </dl>
        </>
      ) : null}
    </div>
  );
}

/**
 * 【Jev】【资源详情】Skill 展开后按名称读取 SKILL.md 正文。
 *
 * 只在首次展开时挂载，因此不会为折叠的条目发起请求。
 *
 * @param props Skill 条目
 * @returns 说明与文档正文；读取失败时退回说明
 */
export function JevSkillDetail({ item }: { item: JevExposedResource }) {
  const { t } = useI18n();
  const document = useQuery({
    queryKey: ["skill-document", item.name],
    queryFn: () => api.skills.document(item.name),
    staleTime: 60_000,
    retry: false
  });
  const description = item.description || document.data?.description || "";
  const body = document.data ? parseSkillDocument(document.data.content).body.trim() : "";
  return (
    <div className="jev-resource-detail">
      {description ? <p className="jev-resource-description">{description}</p> : null}
      {document.isPending ? <p className="jev-resource-empty">{t("Loading Skill…", "正在读取 Skill…")}</p> : null}
      {document.isError ? (
        <p className="jev-resource-empty">{t("Could not read the Skill document.", "无法读取 Skill 文档。")}</p>
      ) : null}
      {body ? <pre className="disclosure-item-text jev-resource-document">{body}</pre> : null}
    </div>
  );
}
