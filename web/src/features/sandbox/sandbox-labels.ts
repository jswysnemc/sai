import type { SandboxDenial, SandboxScope } from "../../api/contracts";

type Translate = (en: string, zh: string) => string;

/**
 * 【沙箱】【范围文案】把权限请求的沙箱范围转换为一行说明与色调。
 * @param scope 沙箱范围
 * @param t 双语文本函数
 * @returns 说明文本与是否需要醒目提示
 */
export function sandboxScopeText(scope: SandboxScope, t: Translate): { text: string; emphasis: boolean } {
  const network = scope.network ? t("network allowed", "允许网络") : t("network blocked", "断开网络");
  const head = {
    sandboxed: `${t("Runs in sandbox", "在沙箱内执行")} ${scope.backend} · ${network}`,
    escalated: t("Leaves the sandbox if approved: full filesystem and network", "批准后在沙箱外执行：完整文件系统与网络权限"),
    unsandboxed: t("No sandbox: runs with your full permissions", "未启用沙箱：以你的完整权限执行"),
    unavailable: t("Sandbox backend unavailable: the command will fail", "沙箱后端不可用：命令会执行失败")
  }[scope.kind];
  const reasons = (scope.reasons ?? []).map((reason) => escapeReasonLabel(reason, t));
  return {
    text: reasons.length ? `${head} · ${t("reason: ", "原因：")}${reasons.join(t(", ", "、"))}` : head,
    emphasis: scope.kind === "escalated" || scope.kind === "unavailable"
  };
}

/**
 * 返回提升原因的可读名称。
 * @param reason 原因标识
 * @param t 双语文本函数
 * @returns 可读名称
 */
export function escapeReasonLabel(reason: string, t: Translate): string {
  switch (reason) {
    case "requested": return t("requested by model", "模型申请");
    case "network": return t("network", "联网");
    case "package_manager": return t("package manager", "包管理器");
    case "outside_path": return t("path outside workspace", "工作区外路径");
    default: return reason;
  }
}

/**
 * 从命令结果中读取沙箱拦截说明。
 * @param result 已解析的命令结果对象
 * @returns 拦截说明；不存在或格式不符时为空
 */
export function sandboxDenialOf(result: Record<string, unknown> | null | undefined): SandboxDenial | null {
  const denial = result?.sandbox_denial;
  if (!denial || typeof denial !== "object") return null;
  const record = denial as Record<string, unknown>;
  const kind = record.kind === "network" ? "network" : record.kind === "filesystem" ? "filesystem" : null;
  if (!kind) return null;
  return {
    kind,
    evidence: typeof record.evidence === "string" ? record.evidence : undefined,
    hint: typeof record.hint === "string" ? record.hint : undefined
  };
}
