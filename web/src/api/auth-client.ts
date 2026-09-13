import { detectInitialLocale, text } from "../features/i18n/locale";

/**
 * 【网页认证】【查询方式】读取服务端当前启用的认证方式。
 * @returns 口令认证与匿名访问是否启用
 */
export async function fetchAuthMode(): Promise<{ password_required: boolean; allow_anonymous: boolean }> {
  const response = await fetch("/api/auth/mode", { credentials: "same-origin" });
  if (!response.ok) return { password_required: false, allow_anonymous: false };
  return (await response.json()) as { password_required: boolean; allow_anonymous: boolean };
}

/**
 * 【网页认证】【口令登录】使用访问口令建立同源会话。
 * @param password 用户输入的访问口令
 * @returns 登录完成，口令无效时抛出错误
 */
export async function loginWithPassword(password: string): Promise<void> {
  const response = await fetch("/api/auth/password", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ password })
  });
  if (!response.ok) {
    throw new Error(text(detectInitialLocale(), "Incorrect password", "口令不正确"));
  }
}

/**
 * 【网页认证】【检查会话】判断当前浏览器会话是否已通过验证。
 * @returns 当前会话能否访问工作区接口
 */
export async function hasActiveSession(): Promise<boolean> {
  const response = await fetch("/api/workspaces", { credentials: "same-origin" });
  return response.ok;
}

/**
 * 【网页认证】【启动会话】使用 URL 启动令牌建立同源会话。
 * @returns 会话建立完成，无法使用其他认证方式时抛出错误
 */
export async function bootstrapSession(): Promise<void> {
  const url = new URL(window.location.href);
  const token = url.searchParams.get("token");
  if (!token) return;
  const response = await fetch(`/api/auth/session?token=${encodeURIComponent(token)}`, {
    method: "POST",
    credentials: "same-origin"
  });
  // 1. 【网页认证】【启动会话】启用口令验证时令牌不再单独放行，失败交由登录页接管
  if (response.ok) {
    url.searchParams.delete("token");
    window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
    return;
  }
  const mode = await fetchAuthMode().catch(() => ({ password_required: false, allow_anonymous: false }));
  if (mode.password_required || mode.allow_anonymous) return;
  throw new Error(text(detectInitialLocale(), "The Sai Web access token is invalid", "Sai Web 访问令牌无效"));
}
