import { useEffect, useRef, useState } from "react";
import { toDisplayError } from "../../../api/api-error";

type ProbeState<Result, Mode> = { identity: string; mode: Mode | null; report: Result | null; error: string | null };

/**
 * 【设置】【草稿探测】隔离不同配置的请求，避免过期结果显示为当前接入状态。
 * @param identity 当前探测配置的稳定标识，仅保存在内存中
 * @param request 指定模式的请求函数
 * @param failure 探测异常的英文与中文提示
 * @returns 当前报告、错误、运行模式与探测操作
 */
export function useDraftProbe<Result, Mode extends string>(identity: string, request: (mode: Mode) => Promise<Result>, failure: [string, string]) {
  const [state, setState] = useState<ProbeState<Result, Mode>>({ identity, mode: null, report: null, error: null });
  const sequence = useRef(0);
  const latestIdentity = useRef(identity);
  latestIdentity.current = identity;
  const pending = useRef<{ identity: string; sequence: number } | null>(null);

  useEffect(() => {
    setState({ identity, mode: null, report: null, error: null });
    return () => { sequence.current += 1; pending.current = null; };
  }, [identity]);

  /**
   * 发起一次探测，同一配置的重复触发会合并。
   * @param mode 本次探测模式
   * @returns 当前有效请求的报告，失败或过期时无值
   */
  const run = async (mode: Mode): Promise<Result | undefined> => {
    if (pending.current?.identity === identity) return;
    const current = ++sequence.current;
    pending.current = { identity, sequence: current };
    setState({ identity, mode, report: null, error: null });
    try {
      const report = await request(mode);
      if (current !== sequence.current || identity !== latestIdentity.current) return;
      setState({ identity, mode: null, report, error: null });
      return report;
    } catch (cause) {
      if (current === sequence.current && identity === latestIdentity.current) {
        setState({ identity, mode: null, report: null, error: toDisplayError(cause, ...failure).message });
      }
    } finally {
      if (pending.current?.sequence === current) pending.current = null;
    }
  };

  const active = state.identity === identity ? state : { mode: null, report: null, error: null };
  return { running: active.mode !== null, runningMode: active.mode, report: active.report, error: active.error, run };
}
