import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
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
  normalizeSkillLibraryPage,
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
  const [page, setPage] = useState<SkillLibraryPage>({ kind: "grid" });
  const [filters, setFilters] = useState<SkillLibraryFilters>(INITIAL_SKILL_LIBRARY_FILTERS);
  const [directoryName, setDirectoryName] = useState("");
  const [content, setContent] = useState("");
  const [dirty, setDirty] = useState(false);
  const [view, setView] = useState<SkillsSettingsView>("library");

  const skills = list.data?.skills ?? [];
  const selectedId = page.kind === "detail" ? page.skillId : "";
  const creating = page.kind === "create";

  useEffect(() => {
    if (!list.isFetched || list.isFetching) return;
    setPage((current) => normalizeSkillLibraryPage(current, skills));
  }, [list.isFetched, list.isFetching, skills]);

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
    mutationFn: () => creating
      ? api.skills.create(directoryName.trim(), content)
      : api.skills.update(selectedId, content),
    onSuccess: async (saved) => {
      setDirty(false);
      setPage({ kind: "detail", skillId: saved.skill.id });
      setContent(saved.content);
      cacheManagedSkill(saved.skill);
      queryClient.setQueryData(["managed-skill", saved.skill.id], saved);
      await queryClient.invalidateQueries({ queryKey: ["managed-skills"] });
    }
  });

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
    setPage({ kind: "detail", skillId: id });
    setContent("");
    setDirty(false);
    save.reset();
    toggle.reset();
  };

  /** 返回技能库网格并清理当前编辑状态。 */
  const returnToLibrary = () => {
    setPage({ kind: "grid" });
    setContent("");
    setDirty(false);
    save.reset();
    toggle.reset();
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
            onDirectoryNameChange={(value) => { setDirectoryName(value); setDirty(true); }}
            onContentChange={(value) => { setContent(value); setDirty(true); save.reset(); }}
            onEnabledChange={(enabled) => selectedSkill && toggle.mutate({ id: selectedSkill.id, enabled })}
            onSave={() => void save.mutateAsync().catch(() => undefined)}
          />
        </div>
      )}
    </div>
  );
}
