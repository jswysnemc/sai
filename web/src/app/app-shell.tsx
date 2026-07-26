import { Outlet, useLocation } from "react-router-dom";
import { ErrorBoundary } from "../shared/ui/error-boundary/error-boundary";
import "./app-shell.css";

/**
 * 渲染无全局顶栏的全高应用外壳。
 *
 * 导航入口已下沉到会话侧栏，主内容区占满视口高度。
 * 内容区包了错误边界：某个页面渲染失败时侧栏与路由仍然可用，
 * 不会整页空白。边界以路径为 key，切换页面时自动重置。
 *
 * @returns 应用外壳布局
 */
export function AppShell() {
  const location = useLocation();
  return (
    <div className="app-shell">
      <main className="app-content">
        <ErrorBoundary key={location.pathname}>
          <Outlet />
        </ErrorBoundary>
      </main>
    </div>
  );
}
