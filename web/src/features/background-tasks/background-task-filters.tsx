import { SegmentedControl } from "../../shared/ui/segmented-control";
import type { BackgroundTask } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";
import { isBackgroundTaskRunning, type BackgroundTaskFilter } from "./background-task-utils";

/**
 * 渲染带实时数量的任务状态筛选，保留浏览器 Tab 焦点导航。
 * @param props 任务集合、当前筛选和变更回调
 * @returns 统一分段筛选控件
 */
export function BackgroundTaskFilters({ tasks, filter, onChange }: {
  tasks: BackgroundTask[]; filter: BackgroundTaskFilter; onChange: (value: BackgroundTaskFilter) => void;
}) {
  const { t } = useI18n();
  const running = tasks.filter(isBackgroundTaskRunning).length;
  return <SegmentedControl value={filter} onChange={onChange} ariaLabel={t("Filter tasks", "任务筛选")}
    className="background-task-filters" options={[
      { value: "all", label: `${t("All", "全部")} ${tasks.length}` },
      { value: "running", label: `${t("Running", "运行中")} ${running}` },
      { value: "finished", label: `${t("Finished", "已结束")} ${tasks.length - running}` }
    ]} />;
}
