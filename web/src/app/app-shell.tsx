import { Outlet } from "react-router-dom";
import "./app-shell.css";

/**
 * 渲染无全局顶栏的全高应用外壳。
 *
 * 导航入口已下沉到会话侧栏，主内容区占满视口高度。
 *
 * @returns 应用外壳布局
 */
export function AppShell() {
  return (
    <div className="app-shell">
      <main className="app-content">
        <Outlet />
      </main>
    </div>
  );
}
