import { useI18n } from "../../features/i18n/use-i18n";
import { SkeletonText } from "./skeleton/skeleton";

/**
 * 为异步页面和功能面板提供统一的紧凑加载占位。
 * @returns 带无障碍加载说明的骨架区域
 */
export function LoadingPanel() {
  const { t } = useI18n();
  return (
    <div className="flex min-h-20 w-full flex-1 items-center justify-center p-4 sm:p-6">
      <div className="w-full max-w-xs">
        <SkeletonText label={t("Loading", "正在加载")} lines={3} />
      </div>
    </div>
  );
}
