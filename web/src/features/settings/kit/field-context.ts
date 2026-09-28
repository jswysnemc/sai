import { createContext, useContext } from "react";

/** 字段上下文：控件据此自动关联标签、说明与错误状态。 */
export type FieldContextValue = {
  /** 控件元素标识，标签的 htmlFor 指向它 */
  controlId: string;
  /** 标签元素标识 */
  labelId: string;
  /** 说明或错误文字的元素标识；无说明时为 undefined */
  hintId?: string;
  /** 字段是否处于错误状态 */
  invalid: boolean;
  /** 标签纯文本，供按钮类控件作为可访问名称 */
  labelText?: string;
};

export const FieldContext = createContext<FieldContextValue | null>(null);

/**
 * 读取最近一层字段上下文。
 *
 * @returns 字段上下文；控件不在字段内时返回 null
 */
export function useFieldContext(): FieldContextValue | null {
  return useContext(FieldContext);
}
