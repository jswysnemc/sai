import type { ReactNode } from "react";
import type { ActionMenuItem } from "../../shared/ui/menu/action-menu";

/** 右键菜单落点：条目菜单带路径，空白处菜单不带。 */
export type FilesHomeMenuTarget = {
  x: number;
  y: number;
  path?: string;
  directory?: boolean;
  /** 条目来自最近列表时才可移出 */
  recent?: boolean;
};

type Translate = (en: string, zh: string) => string;

type FilesHomeMenuActions = {
  open: (path: string, directory: boolean) => void;
  reveal: (path: string) => void;
  copyPath: (path: string, absolute: boolean) => void;
  forget: (path: string) => void;
  createFile: () => void;
  browse: () => void;
  clearRecents: () => void;
  hasRecents: boolean;
  icons: { open: ReactNode; reveal: ReactNode; copy: ReactNode; remove: ReactNode; create: ReactNode; browse: ReactNode; clear: ReactNode };
};

/**
 * 【工作区】【Files 首页】按右键落点生成菜单项。
 *
 * 条目：打开、在文件树中定位、复制路径、复制相对路径、从最近移除。
 * 空白：新建文件、浏览文件、清空最近记录。
 *
 * @param target 右键落点
 * @param actions 菜单动作与图标
 * @param t 双语文本
 * @returns 菜单项
 */
export function filesHomeMenuItems(target: FilesHomeMenuTarget, actions: FilesHomeMenuActions, t: Translate): ActionMenuItem[] {
  const { path, directory = false } = target;
  if (path) {
    return [
      { id: "open", label: directory ? t("Open in File Tree", "在文件树中展开") : t("Open", "打开"), icon: actions.icons.open, onSelect: () => actions.open(path, directory) },
      ...(!directory ? [{ id: "reveal", label: t("Reveal in File Tree", "在文件树中定位"), icon: actions.icons.reveal, onSelect: () => actions.reveal(path) }] : []),
      { id: "copy-path", label: t("Copy Path", "复制路径"), icon: actions.icons.copy, separator: true, onSelect: () => actions.copyPath(path, true) },
      { id: "copy-relative", label: t("Copy Relative Path", "复制相对路径"), onSelect: () => actions.copyPath(path, false) },
      ...(target.recent ? [{ id: "forget", label: t("Remove from Recents", "从最近中移除"), icon: actions.icons.remove, separator: true, onSelect: () => actions.forget(path) }] : [])
    ];
  }
  return [
    { id: "new-file", label: t("New File", "新建文件"), icon: actions.icons.create, onSelect: actions.createFile },
    { id: "browse", label: t("Browse Files", "浏览文件"), icon: actions.icons.browse, onSelect: actions.browse },
    { id: "clear", label: t("Clear Recents", "清空最近记录"), icon: actions.icons.clear, separator: true, disabled: !actions.hasRecents, danger: true, onSelect: actions.clearRecents }
  ];
}
