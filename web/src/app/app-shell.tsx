import { Outlet, useLocation } from "react-router-dom";
import { ErrorBoundary } from "../shared/ui/error-boundary/error-boundary";
import { appViewKey } from "./app-view-key";
import "./app-shell.css";

/**
 * 渲染无全局顶栏的全高应用外壳。
 *
 * 导航入口已下沉到会话侧栏，主内容区占满视口高度。
 * 内容区包了错误边界：某个页面渲染失败时侧栏与路由仍然可用，
 * 不会整页空白。边界以顶层路径段为 key，切换页面时自动重置。
 *
 * 同一个 key 也驱动进场动画：切换顶层页面时容器重新挂载，
 * CSS 动画随之重放。key 不含子路径，否则设置页切换分区或子页会整页
 * 重挂载，页面持有的编辑草稿随之丢失。
 *
 * @returns 应用外壳布局
 */
export function AppShell() {
  const location = useLocation();
  return (
    <div className="app-shell">
      <main className="app-content">
        <div className="app-view" key={appViewKey(location.pathname)}>
          <ErrorBoundary>
            <Outlet />
          </ErrorBoundary>
        </div>
      </main>
    </div>
  );
}
