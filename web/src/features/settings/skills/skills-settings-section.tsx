import { useSettingsDraft } from "../shell/settings-draft-context";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { ManagedSkill } from "../../../api/skill-contracts";
import type { AppConfig } from "../../../api/contracts";
import { SkillBehaviorSettings } from "../runtime/skill-behavior-settings";
import { useI18n } from "../../i18n/use-i18n";
import { SkillDetailView } from "./skill-detail-view";
import { SkillGrid } from "./skill-grid";
import { SkillsSettingsTabs, type SkillsSettingsView } from "./skills-settings-tabs";
import {
  INITIAL_SKILL_LIBRARY_FILTERS,
  type SkillLibraryFilters,
  type SkillLibraryPage
} from "./skill-view-state";
import "./skills-settings.css";

const SKILL_TEMPLATE = `---
name: new-skill
description: Describe when this Skill should be used
---

# Instructions

Describe the workflow, constraints, and expected output.
`;

type SkillsSettingsSectionProps = {
  config?: AppConfig | null;
  onConfigChange?: (config: AppConfig) => void;
};

/**
 * 编排 Skills 行为配置与文档扫描、读取、新增、编辑与启停。
 *
 * @param props 可选 AppConfig（用于技能行为字段）
 * @returns Skills 设置页面
 */
export function SkillsSettingsSection({ config, onConfigChange }: SkillsSettingsSectionProps = {}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: ["managed-skills"], queryFn: api.skills.managedList });
  const [params, setParams] = useSearchParams();
  const requested = params.get("item");
  const page: SkillLibraryPage = params.get("create") === "1" ? { kind: "create" } : requested ? { kind: "detail", skillId: requested } : { kind: "grid" };
  /** 同步技能详情与创建状态到地址；参数为目标页面，返回无值。 */
  const setPage = (next: SkillLibraryPage) => setParams((current) => {
    const updated = new URLSearchParams(current);
    updated.delete("item");
    updated.delete("create");
    if (next.kind === "detail") updated.set("item", next.skillId);
    if (next.kind === "create") updated.set("create", "1");
    return updated;
  }, { replace: true });
  const [filters, setFilters] = useState<SkillLibraryFilters>(INITIAL_SKILL_LIBRARY_FILTERS);
  const [directoryName, setDirectoryName] = useState("");
  const [content, setContent] = useState("");
  const [dirty, setDirty] = useState(false);
  const revision = useRef(0);
  const generation = useRef(0);
  const savedTarget = useRef<string | null>(null);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const view: SkillsSettingsView = params.get("view") === "behavior" ? "behavior" : "library";
  /** 同步技能库视图；参数为视图名称，返回无值。 */
  const setView = (view: SkillsSettingsView) => setParams((current) => { const next = new URLSearchParams(current); next.set("view", view); return next; }, { replace: true });

  const skills = list.data?.skills ?? [];
  const selectedId = page.kind === "detail" ? page.skillId : "";
  const creating = page.kind === "create";

  useEffect(() => {
    if (!list.isFetched || list.isFetching) return;
    if (requested && !skills.some((skill) => skill.id === requested)) {
      setParams((current) => { const next = new URLSearchParams(current); next.delete("item"); return next; }, { replace: true });
    }
  }, [list.isFetched, list.isFetching, skills, requested, setParams]);

  const document = useQuery({
    queryKey: ["managed-skill", selectedId],
    queryFn: () => api.skills.managedDocument(selectedId),
    enabled: Boolean(selectedId) && !creating
  });

  useEffect(() => {
    if (!document.data || dirty || creating) return;
    setContent(document.data.content);
  }, [creating, dirty, document.data, selectedId]);

  /**
   * 将新增或更新后的 Skill 合并到技能库查询缓存。
   *
   * @param skill 后端返回的最新 Skill
   * @returns 无返回值
   */
  const cacheManagedSkill = (skill: ManagedSkill) => {
    queryClient.setQueryData<{ skills: ManagedSkill[] }>(["managed-skills"], (current) => {
      if (!current) return { skills: [skill] };
      const index = current.skills.findIndex((item) => item.id === skill.id);
      if (index < 0) return { skills: [...current.skills, skill] };
      return { skills: current.skills.map((item) => item.id === skill.id ? skill : item) };
    });
  };

  const save = useMutation({
    mutationFn: async () => {
      const submittedRevision = revision.current;
      const submittedGeneration = generation.current;
      const saved = await (creating
        ? api.skills.create(directoryName.trim(), content)
        : api.skills.update(selectedId, content));
      return { saved, submittedRevision, submittedGeneration };
    },
    onSuccess: async ({ saved, submittedRevision, submittedGeneration }) => {
      cacheManagedSkill(saved.skill);
      queryClient.setQueryData(["managed-skill", saved.skill.id], saved);
      if (mounted.current && generation.current === submittedGeneration) {
        // 1. 【技能文档】【保存回填】请求期间的新内容继续作为草稿，创建成功后允许切换到新文档地址
        if (revision.current === submittedRevision) {
          setDirty(false);
          setContent(saved.content);
        }
        savedTarget.current = saved.skill.id;
        setPage({ kind: "detail", skillId: saved.skill.id });
      }
      await queryClient.invalidateQueries({ queryKey: ["managed-skills"] });
    }
  });

  useSettingsDraft({
    scope: "/settings/skills", dirty, saving: save.isPending,
    discard: () => { revision.current += 1; generation.current += 1; setDirty(false); },
    save: () => save.mutateAsync(),
    contains: (search) => {
      const target = new URLSearchParams(search);
      if (savedTarget.current && target.get("item") === savedTarget.current && !target.has("create")) return true;
      return (target.get("item") ?? "") === selectedId && (target.get("create") === "1") === creating;
    }
  });
  useEffect(() => { savedTarget.current = null; }, [selectedId, creating]);

  const toggle = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => api.skills.setEnabled(id, enabled),
    onSuccess: async (saved) => {
      cacheManagedSkill(saved.skill);
      queryClient.setQueryData(["managed-skill", saved.skill.id], saved);
      await queryClient.invalidateQueries({ queryKey: ["managed-skills"] });
    }
  });

  /** 进入新建状态并填充最小有效模板。 */
  const startCreating = () => {
    generation.current += 1;
    setPage({ kind: "create" });
    setDirectoryName("");
    setContent(SKILL_TEMPLATE);
    setDirty(true);
    save.reset();
    toggle.reset();
  };

  /**
   * 打开已有 Skill 的详情设置界面。
   *
   * @param id Skill 唯一标识
   * @returns 无返回值
   */
  const selectSkill = (id: string) => {
    generation.current += 1;
    setPage({ kind: "detail", skillId: id });
    setContent("");
    setDirty(false);
    save.reset();
    toggle.reset();
  };

  /** 返回技能库网格并清理当前编辑状态。 */
  const returnToLibrary = () => {
    if (save.isPending) return;
    setPage({ kind: "grid" });
  };

  const selectedSkill: ManagedSkill | null = creating
    ? null
    : (document.data?.skill ?? skills.find((skill) => skill.id === selectedId) ?? null);
  const listError = list.error
    ? toDisplayError(list.error, "Skills scan error", "Skills 扫描错误").message
    : null;
  const editorRequestError = document.error ?? save.error ?? toggle.error;
  const editorError = editorRequestError
    ? toDisplayError(editorRequestError, "Skills management error", "Skills 管理错误").message
    : null;

  return (
    <div className="skills-settings-page">
      <SkillsSettingsTabs
        value={view}
        total={skills.length}
        enabled={skills.filter((skill) => skill.enabled).length}
        onChange={setView}
      />
      {view === "behavior" ? (
        <div id="skills-behavior-panel" className="skills-behavior-panel" role="tabpanel">
          {config && onConfigChange
            ? <SkillBehaviorSettings config={config} onConfigChange={onConfigChange} />
            : <div className="settings-state">{t("Runtime policy is unavailable", "运行策略当前不可用")}</div>}
        </div>
      ) : list.isLoading ? (
        <div id="skills-library-panel" className="skills-library-loading" role="tabpanel" aria-label={t("Scanning Skills", "正在扫描 Skills")}>
          <span /><span /><span /><span /><span /><span />
        </div>
      ) : page.kind === "grid" ? (
        <div id="skills-library-panel" role="tabpanel">
          <SkillGrid
            skills={skills}
            filters={filters}
            scanning={list.isFetching}
            error={listError}
            onFiltersChange={setFilters}
            onOpen={selectSkill}
            onAdd={startCreating}
            onScan={() => void list.refetch()}
          />
        </div>
      ) : (
        <div id="skills-library-panel" role="tabpanel">
          <SkillDetailView
            skill={selectedSkill}
            content={content}
            directoryName={directoryName}
            creating={creating}
            dirty={dirty}
            saving={save.isPending}
            loading={!creating && document.isLoading}
            error={editorError}
            onBack={returnToLibrary}
            onDirectoryNameChange={(value) => { revision.current += 1; setDirectoryName(value); setDirty(true); }}
            onContentChange={(value) => {
              if (value === content) return;
              revision.current += 1;
              setContent(value);
              setDirty(true);
              if (!save.isPending) save.reset();
            }}
            onEnabledChange={(enabled) => selectedSkill && toggle.mutate({ id: selectedSkill.id, enabled })}
            onSave={() => void save.mutateAsync().catch(() => undefined)}
          />
        </div>
      )}
    </div>
  );
}
