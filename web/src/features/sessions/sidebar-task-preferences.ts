export type TaskView = "project" | "grouped" | "timeline" | "archived";
export type TaskSort = "updated" | "created";
export type PurposeSectionId = "projects" | "tasks";

export type SidebarTaskPreferences = {
  view: TaskView;
  sort: TaskSort;
  sectionOrder: PurposeSectionId[];
  projectsOpen: boolean;
  tasksOpen: boolean;
};

const STORAGE_KEY = "sai.sidebar.task-view";

const DEFAULTS: SidebarTaskPreferences = {
  view: "project",
  sort: "updated",
  sectionOrder: ["projects", "tasks"],
  projectsOpen: true,
  tasksOpen: true
};

/**
 * 读取左栏视图、排序和分区顺序。
 *
 * @returns 侧栏任务偏好
 */
export function readSidebarTaskPreferences(): SidebarTaskPreferences {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULTS, sectionOrder: [...DEFAULTS.sectionOrder] };
    const parsed = JSON.parse(raw) as Partial<SidebarTaskPreferences>;
    const order = parsed.sectionOrder?.filter((id): id is PurposeSectionId => id === "projects" || id === "tasks");
    return {
      view: parsed.view === "grouped" || parsed.view === "timeline" || parsed.view === "archived" ? parsed.view : "project",
      sort: parsed.sort === "created" ? "created" : "updated",
      sectionOrder: order?.length === 2 ? order : [...DEFAULTS.sectionOrder],
      projectsOpen: parsed.projectsOpen ?? true,
      tasksOpen: parsed.tasksOpen ?? true
    };
  } catch {
    return { ...DEFAULTS, sectionOrder: [...DEFAULTS.sectionOrder] };
  }
}

/**
 * 写入左栏任务偏好。
 *
 * @param preferences 视图、排序和分区顺序
 */
export function persistSidebarTaskPreferences(preferences: SidebarTaskPreferences): void {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
}
