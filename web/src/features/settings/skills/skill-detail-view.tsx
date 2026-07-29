import { ArrowLeft } from "lucide-react";
import type { ManagedSkill } from "../../../api/skill-contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { SkillEditor } from "./skill-editor";
import "./skill-detail-view.css";

type SkillDetailViewProps = {
  skill: ManagedSkill | null;
  content: string;
  directoryName: string;
  creating: boolean;
  dirty: boolean;
  saving: boolean;
  loading: boolean;
  error: string | null;
  onBack: () => void;
  onContentChange: (content: string) => void;
  onDirectoryNameChange: (name: string) => void;
  onEnabledChange: (enabled: boolean) => void;
  onSave: () => void;
};

/**
 * 渲染 Skill 独立详情设置页，并提供返回技能库入口。
 *
 * @param props Skill 文档、编辑状态与操作回调
 * @returns Skill 详情设置界面
 */
export function SkillDetailView({ loading, onBack, ...editorProps }: SkillDetailViewProps) {
  const { t } = useI18n();

  return (
    <section className="skill-detail-view" aria-label={t("Skill details", "Skill 详情")}>
      <div className="skill-detail-navigation">
        <Button className="skill-detail-back" onClick={onBack}>
          <ArrowLeft size={14} />
          {t("Back to library", "返回技能库")}
        </Button>
        <span>{editorProps.creating ? t("New Skill", "新增 Skill") : (editorProps.skill?.name ?? t("Skill details", "Skill 详情"))}</span>
      </div>
      {loading ? (
        <div className="skill-detail-loading" aria-label={t("Loading Skill", "正在加载 Skill")}>
          <span /><span /><span />
        </div>
      ) : (
        <SkillEditor {...editorProps} />
      )}
    </section>
  );
}
