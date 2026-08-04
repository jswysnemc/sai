import { useRef, useState } from "react";
import { parsePickedDirectory, parsePickedDirectoryName } from "./picked-directory";

/** 浏览器目录选择的结果。 */
export type DirectoryPickResult = {
  /** 用户选中的目录名 */
  name: string;
  /** 与服务端允许根拼接后的候选绝对路径 */
  candidates: string[];
};

/** 目录选择器不可用或用户取消时的状态。 */
export type DirectoryPickState =
  | { status: "idle" }
  | { status: "picking" }
  | { status: "resolved"; result: DirectoryPickResult }
  | { status: "unsupported" };

/**
 * File System Access API 的目录句柄，仅取用到的字段。
 *
 * TypeScript 内置 DOM 类型尚未覆盖 showDirectoryPicker，这里做最小声明。
 */
type DirectoryHandleLike = { name: string };

type WindowWithDirectoryPicker = Window & {
  showDirectoryPicker?: (options?: { mode?: string }) => Promise<DirectoryHandleLike>;
};

/**
 * 调起系统文件选择器让用户挑一个目录，并还原为服务端可用的绝对路径候选。
 *
 * 浏览器出于安全考虑不交出绝对路径：File System Access API 只给目录名，
 * `<input webkitdirectory>` 只给相对路径。两条路径都只能拿到目录名，
 * 再与服务端允许根拼接还原。因此只有位于允许根之下的目录能被定位。
 *
 * @param roots 服务端允许浏览的根目录路径
 * @returns 选择状态、发起选择的方法、隐藏 input 的挂载属性
 */
export function useDirectoryPicker(roots: string[]) {
  const [state, setState] = useState<DirectoryPickState>({ status: "idle" });
  const inputRef = useRef<HTMLInputElement | null>(null);

  /** 用目录名解析出候选路径并落入状态。 */
  const resolveName = (name: string) => {
    const parsed = parsePickedDirectoryName(name, roots);
    setState(parsed.name ? { status: "resolved", result: parsed } : { status: "idle" });
  };

  /**
   * 发起目录选择。
   *
   * 1. 优先用 File System Access API，它是真正的系统目录选择器
   * 2. 不支持时回退到隐藏的 `<input webkitdirectory>`
   */
  const pick = async () => {
    const picker = (window as WindowWithDirectoryPicker).showDirectoryPicker;
    if (typeof picker === "function") {
      setState({ status: "picking" });
      try {
        const handle = await picker({ mode: "read" });
        resolveName(handle.name);
      } catch {
        // 用户取消选择不是错误，回到初始状态
        setState({ status: "idle" });
      }
      return;
    }
    if (!inputRef.current) {
      setState({ status: "unsupported" });
      return;
    }
    setState({ status: "picking" });
    inputRef.current.value = "";
    inputRef.current.click();
  };

  /** 处理 `<input webkitdirectory>` 的选择结果。 */
  const handleInputChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const paths = Array.from(event.target.files ?? []).map(
      (file) => (file as File & { webkitRelativePath?: string }).webkitRelativePath ?? ""
    );
    const parsed = parsePickedDirectory(paths, roots);
    setState(parsed.name ? { status: "resolved", result: parsed } : { status: "idle" });
  };

  /** 清空选择结果。 */
  const reset = () => setState({ status: "idle" });

  return { state, pick, reset, inputRef, handleInputChange };
}
