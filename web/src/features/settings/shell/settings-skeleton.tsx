import { useI18n } from "../../i18n/use-i18n";
import "./settings-skeleton.css";

type SettingsSkeletonProps = {
  /** 骨架行数，默认 5 */
  rows?: number;
};

/**
 * 设置分区加载骨架屏，模拟表单布局减少空白闪烁。
 *
 * @param props 骨架配置
 * @returns 骨架占位元素
 */
export function SettingsSkeleton({ rows = 5 }: SettingsSkeletonProps) {
  const { t } = useI18n();
  return (
    <div className="settings-skeleton" aria-label={t("Loading configuration", "正在读取配置")} role="status">
      {/* 标题占位 */}
      <div className="skeleton-line skeleton-title" />
      {Array.from({ length: rows }, (_, i) => (
        <div className="skeleton-block" key={i}>
          <div className="skeleton-line skeleton-label" />
          <div className="skeleton-line skeleton-field" />
        </div>
      ))}
    </div>
  );
}
