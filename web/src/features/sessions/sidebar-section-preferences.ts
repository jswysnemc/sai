export type SidebarSectionId = "projects" | "sessions" | "files";

const STORAGE_KEY = "sai.sidebar.sections";

const DEFAULT_OPEN: Record<SidebarSectionId, boolean> = {
  projects: true,
  sessions: true,
  files: false
};

/**
 * 读取左栏分区的展开偏好。
 *
 * 缺省或损坏的存储按默认展开处理，避免一次坏数据把整个侧栏收起。
 *
 * @returns 各分区是否展开
 */
export function readSidebarSectionPreferences(): Record<SidebarSectionId, boolean> {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_OPEN };
    const parsed = JSON.parse(raw) as Partial<Record<SidebarSectionId, boolean>>;
    return {
      projects: parsed.projects ?? DEFAULT_OPEN.projects,
      sessions: parsed.sessions ?? DEFAULT_OPEN.sessions,
      files: parsed.files ?? DEFAULT_OPEN.files
    };
  } catch {
    return { ...DEFAULT_OPEN };
  }
}

/**
 * 写入左栏分区的展开偏好。
 *
 * @param preferences 各分区是否展开
 */
export function persistSidebarSectionPreferences(preferences: Record<SidebarSectionId, boolean>): void {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
}
