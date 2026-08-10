/**
 * 由路径推导应用视图的重挂载键。
 *
 * 外壳以此为 key 重置错误边界并重放进场动画。取顶层路径段而非完整路径：
 * 设置页的分区与子页切换属于页内导航，整页重挂载会销毁页面持有的编辑草稿。
 *
 * @param pathname 当前路径
 * @returns 顶层视图标识
 */
export function appViewKey(pathname: string): string {
  const segment = pathname.split("/").find((part) => part !== "");
  return segment ?? "";
}
