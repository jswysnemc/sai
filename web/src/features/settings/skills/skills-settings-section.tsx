import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../../api/client";
import { toDisplayError } from "../../../api/api-error";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { SkillEditor } from "./skill-editor";
import { SkillListPanel } from "./skill-list-panel";
import type { AppConfig } from "../../../api/contracts";
import { SkillBehaviorSettings } from "../runtime/skill-behavior-settings";
import { useI18n } from "../../i18n/use-i18n";
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
  const [selectedId, setSelectedId] = useState("");
  const [creating, setCreating] = useState(false);
  const [directoryName, setDirectoryName] = useState("");
  const [content, setContent] = useState("");
  const [dirty, setDirty] = useState(false);

  const skills = list.data?.skills ?? [];
  useEffect(() => {
    if (creating) return;
    if (!skills.some((skill) => skill.id === selectedId)) {
      setSelectedId(skills[0]?.id ?? "");
    }
  }, [creating, selectedId, skills]);

  const document = useQuery({
    queryKey: ["managed-skill", selectedId],
    queryFn: () => api.skills.managedDocument(selectedId),
    enabled: Boolean(selectedId) && !creating
  });

  useEffect(() => {
    if (!document.data || dirty || creating) return;
    setContent(document.data.content);
  }, [creating, dirty, document.data]);

  const save = useMutation({
    mutationFn: () => creating
      ? api.skills.create(directoryName.trim(), content)
      : api.skills.update(selectedId, content),
    onSuccess: async (saved) => {
      setCreating(false);
      setDirty(false);
      setSelectedId(saved.skill.id);
      setContent(saved.content);
      queryClient.setQueryData(["managed-skill", saved.skill.id], saved);
      await queryClient.invalidateQueries({ queryKey: ["managed-skills"] });
    }
  });

  const toggle = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => api.skills.setEnabled(id, enabled),
    onSuccess: async (saved) => {
      queryClient.setQueryData(["managed-skill", saved.skill.id], saved);
      await queryClient.invalidateQueries({ queryKey: ["managed-skills"] });
    }
  });

  /** 进入新建状态并填充最小有效模板。 */
  const startCreating = () => {
    setCreating(true);
    setSelectedId("");
    setDirectoryName("");
    setContent(SKILL_TEMPLATE);
    setDirty(true);
    save.reset();
  };

  /** 选择已有 Skill，并放弃当前未保存的新建草稿。 */
  const selectSkill = (id: string) => {
    setCreating(false);
    setSelectedId(id);
    setDirty(false);
    save.reset();
  };

  const selectedSkill: ManagedSkill | null = creating
    ? null
    : (document.data?.skill ?? skills.find((skill) => skill.id === selectedId) ?? null);
  const requestError = list.error ?? document.error ?? save.error ?? toggle.error;
  const error = requestError
    ? toDisplayError(requestError, "Skills management error", "Skills 管理错误").message
    : null;

  if (list.isLoading) {
    return <div className="settings-state">{t("Scanning Skills", "正在扫描 Skills")}</div>;
  }

  return (
    <div className="skills-settings-page">
      {config && onConfigChange && (
        <SkillBehaviorSettings config={config} onConfigChange={onConfigChange} />
      )}
    <div className="settings-objects-layout skills-settings-layout">
      <SkillListPanel
        skills={skills}
        selectedId={selectedId}
        scanning={list.isFetching}
        onSelect={selectSkill}
        onAdd={startCreating}
        onScan={() => void list.refetch()}
      />
      <SkillEditor
        skill={selectedSkill}
        content={content}
        directoryName={directoryName}
        creating={creating}
        dirty={dirty}
        saving={save.isPending}
        error={error}
        onDirectoryNameChange={(value) => { setDirectoryName(value); setDirty(true); }}
        onContentChange={(value) => { setContent(value); setDirty(true); save.reset(); }}
        onEnabledChange={(enabled) => selectedSkill && toggle.mutate({ id: selectedSkill.id, enabled })}
        onSave={() => void save.mutateAsync().catch(() => undefined)}
      />
    </div>
    </div>
  );
}
