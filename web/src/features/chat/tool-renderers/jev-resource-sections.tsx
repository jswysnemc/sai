import { useI18n } from "../../i18n/use-i18n";
import { DisclosureItem } from "./disclosure-item";
import type { JevCapabilityExposure, JevExposedResource } from "./jev-capability-data";
import { JevSkillDetail, JevToolDetail } from "./jev-resource-detail";
import { contextSourceLabel, type JevInjectedContext } from "./jev-context-data";

type JevResourceSectionsProps = {
  exposure: JevCapabilityExposure;
};

/**
 * 【Jev】【资源分区】按工具、Skill、提示词片段、记忆四类列出本轮结果。
 *
 * 每条都可展开：工具显示完整说明与参数，Skill 读取 SKILL.md 正文，
 * 片段与记忆显示注入正文的预览。
 *
 * @param props 暴露名单
 * @returns 四个分区；空分区不渲染
 */
export function JevResourceSections({ exposure }: JevResourceSectionsProps) {
  const { t } = useI18n();
  const prompts = exposure.contexts.filter((item) => item.kind === "prompt");
  const memories = exposure.contexts.filter((item) => item.kind === "memory");
  return (
    <>
      <ResourceList title={t("Tools", "工具")} items={exposure.tools} detailLabel={(item) => item.detail} />
      <ResourceList
        title="Skills"
        items={exposure.skills}
        detailLabel={(item) => skillStatusLabel(item.detail, t)}
      />
      <ContextList title={t("Prompt segments", "提示词片段")} items={prompts} />
      <ContextList title={t("Memory", "记忆")} items={memories} />
    </>
  );
}

/**
 * 渲染工具或 Skill 名单，每条可展开详情。
 *
 * 详情在首次展开时才挂载：Skill 正文需要请求接口，折叠时不发请求。
 *
 * @param props 标题、条目，以及把 detail 转成界面文字的方法
 * @returns 列表；没有条目时为空
 */
function ResourceList({
  title,
  items,
  detailLabel
}: {
  title: string;
  items: JevExposedResource[];
  detailLabel: (item: JevExposedResource) => string;
}) {
  if (items.length === 0) return null;
  return (
    <section>
      <h3>{title}</h3>
      <ul className="jev-context-list">
        {items.map((item) => (
          <DisclosureItem key={`${item.kind}:${item.name}`} title={item.name} meta={detailLabel(item) || undefined} lazy>
            {item.kind === "tool" ? <JevToolDetail item={item} /> : <JevSkillDetail item={item} />}
          </DisclosureItem>
        ))}
      </ul>
    </section>
  );
}

/**
 * 渲染提示词片段或记忆；每条可展开注入正文预览。
 *
 * @param props 标题与条目
 * @returns 列表；没有条目时为空
 */
function ContextList({ title, items }: { title: string; items: JevInjectedContext[] }) {
  const { t } = useI18n();
  if (items.length === 0) return null;
  return (
    <section>
      <h3>{title}</h3>
      <ul className="jev-context-list">
        {items.map((item, index) => {
          const name = item.kind === "memory"
            ? t("Memory context", "记忆上下文")
            : item.description || t("Untitled segment", "未命名片段");
          const meta = item.kind === "memory"
            ? memoryMeta(item, t)
            : contextSourceLabel(item.source, t);
          return (
            <DisclosureItem key={item.id || `${item.kind}-${index}`} title={name} meta={meta}>
              {item.preview ? <pre className="disclosure-item-text">{item.preview}</pre> : undefined}
            </DisclosureItem>
          );
        })}
      </ul>
    </section>
  );
}

/**
 * 记忆条目的说明：有索引时给出条目数，否则说明只注入了使用说明。
 *
 * @param item 记忆上下文
 * @param t 双语文本选择方法
 * @returns 说明文字
 */
function memoryMeta(item: JevInjectedContext, t: (en: string, zh: string) => string): string {
  const entries = item.preview.split("\n").filter((line) => line.trim().startsWith("-")).length;
  if (entries > 0) return t(`${entries} indexed memories`, `${entries} 条记忆索引`);
  return t("read/write guidance only", "仅记忆使用说明");
}

/**
 * 把 skill 状态码转成界面文字。
 *
 * @param status 后端状态
 * @param t 双语文本选择方法
 * @returns 状态说明；未知状态原样返回
 */
export function skillStatusLabel(status: string, t: (en: string, zh: string) => string): string {
  if (status === "loaded") return t("newly exposed", "新暴露");
  if (status === "already_loaded") return t("already exposed", "此前已暴露");
  return status;
}
