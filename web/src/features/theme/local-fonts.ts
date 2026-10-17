const VISIBLE_FAMILY_LIMIT = 200;

type LocalFontRecord = { family?: string };

type LocalFontQuery = () => Promise<LocalFontRecord[]>;

type DesktopFontCatalog = {
  list?: () => Promise<unknown>;
};

/**
 * 从字体记录里抽出不重复的字族，并按当前语言排序。
 *
 * 同一字族会有 Regular、Bold 等多条记录，选择时只保留字族名。
 *
 * @param fonts 本机字体记录
 * @returns 排序后的字族名
 */
export function uniqueFontFamilies(fonts: Iterable<LocalFontRecord>): string[] {
  const names = new Set<string>();
  for (const font of fonts) {
    const family = font.family?.trim();
    if (family) names.add(family);
  }
  return [...names].sort((left, right) => left.localeCompare(right, "zh"));
}

/**
 * 按关键字筛出要显示的字族。
 *
 * @param families 全部字族
 * @param query 搜索文本
 * @returns 最多 200 条匹配结果，以及是否还有未显示的匹配
 */
export function filterFontFamilies(families: readonly string[], query: string): { visible: string[]; hidden: number } {
  const needle = query.trim().toLocaleLowerCase();
  const matched = needle
    ? families.filter((family) => family.toLocaleLowerCase().includes(needle))
    : [...families];
  return {
    visible: matched.slice(0, VISIBLE_FAMILY_LIMIT),
    hidden: Math.max(0, matched.length - VISIBLE_FAMILY_LIMIT)
  };
}

/**
 * 当前页面能否询问本机字体。
 *
 * @returns Chromium 提供 Local Font Access 时为 true
 */
export function canQueryLocalFonts(): boolean {
  return typeof window !== "undefined" && (localFontQuery() !== undefined || desktopFontCatalog()?.list !== undefined);
}

/**
 * 读取本机已安装字族。
 *
 * 桌面端已预先允许该权限。浏览器里需要由点击触发，否则会拒绝。
 *
 * @returns 字族名；接口不存在时为空数组
 */
export async function readLocalFontFamilies(): Promise<string[]> {
  const query = localFontQuery();
  if (query) {
    try {
      const families = uniqueFontFamilies(await query());
      if (families.length > 0 || !desktopFontCatalog()?.list) return families;
    } catch (error) {
      if (!desktopFontCatalog()?.list) throw error;
    }
  }
  return readDesktopFontFamilies();
}

/**
 * 读取页面上的 Local Font Access 接口。
 *
 * @returns 浏览器提供该接口时返回函数
 */
function localFontQuery(): LocalFontQuery | undefined {
  if (typeof window === "undefined") return undefined;
  return (window as Window & { queryLocalFonts?: LocalFontQuery }).queryLocalFonts;
}

/**
 * 读取桌面端预加载脚本暴露的字库接口。
 *
 * @returns 仅 Electron 工作台页面存在
 */
function desktopFontCatalog(): DesktopFontCatalog | null {
  if (typeof window === "undefined") return null;
  return (window as Window & { saiLocalFonts?: DesktopFontCatalog }).saiLocalFonts ?? null;
}

/**
 * 从桌面端字库接口取出字族名。
 *
 * @returns 字族名；接口缺失时为空数组
 */
async function readDesktopFontFamilies(): Promise<string[]> {
  const list = desktopFontCatalog()?.list;
  if (!list) return [];
  const names = await list();
  if (!Array.isArray(names)) return [];
  return uniqueFontFamilies(names.map((family) => ({ family: typeof family === "string" ? family : "" })));
}
