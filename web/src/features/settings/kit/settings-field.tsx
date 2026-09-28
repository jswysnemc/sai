import { useId, type ReactNode } from "react";
import { fieldAnchorId } from "../search/field-anchor";
import { cx } from "./class-names";
import { FieldContext } from "./field-context";
import "./settings-field.css";

/** 控件最大宽度档位：xs 6rem、sm 10rem、md 16rem、lg 24rem、full 占满单元格。 */
export type ControlSize = "xs" | "sm" | "md" | "lg" | "full";

type SettingsFieldProps = {
  /** 字段标签 */
  label: ReactNode;
  /** 说明文字，自动换行 */
  hint?: ReactNode;
  /** 错误文字；存在时替换说明并把控件标为无效 */
  error?: ReactNode;
  /** 对应的配置键，只出现在标签提示与搜索索引中 */
  configKey?: string;
  /** 搜索定位锚点 */
  anchor?: string;
  /** 控件最大宽度档位 */
  size?: ControlSize;
  /** 为 full 时跨整行 */
  span?: "full";
  /** 紧跟标签的徽标，例如「仅本浏览器」 */
  badge?: ReactNode;
  /** 标签行右侧的辅助操作 */
  aside?: ReactNode;
  className?: string;
  children: ReactNode;
};

/**
 * 渲染设置字段：标签、控件与说明三段式排布。
 *
 * 控件通过字段上下文自动关联标签与说明，调用方无需手写 id。
 *
 * @param props 标签、说明、错误、配置键、锚点、宽度档位与控件
 * @returns 设置字段
 */
export function SettingsField({
  label,
  hint,
  error,
  configKey,
  anchor,
  size = "full",
  span,
  badge,
  aside,
  className,
  children
}: SettingsFieldProps) {
  const baseId = useId();
  const controlId = `${baseId}-control`;
  const labelId = `${baseId}-label`;
  const message = error ?? hint;
  const hintId = message ? `${baseId}-hint` : undefined;
  return (
    <div
      className={cx("sk-field", span === "full" && "is-full", className)}
      id={anchor ? fieldAnchorId(anchor) : undefined}
      data-size={size}
      data-config-key={configKey}
    >
      <div className="sk-field-head">
        <label className="sk-field-label" id={labelId} htmlFor={controlId} title={configKey}>{label}</label>
        {badge}
        {aside && <span className="sk-field-aside">{aside}</span>}
      </div>
      <FieldContext.Provider
        value={{
          controlId,
          labelId,
          hintId,
          invalid: Boolean(error),
          labelText: typeof label === "string" ? label : undefined
        }}
      >
        <div className="sk-field-control">{children}</div>
      </FieldContext.Provider>
      {message && (
        <p className={error ? "sk-field-hint is-error" : "sk-field-hint"} id={hintId}>{message}</p>
      )}
    </div>
  );
}

type FieldGridProps = {
  /** 最多列数；默认两列，短字段较多时用三列 */
  columns?: 1 | 2 | 3;
  className?: string;
  children: ReactNode;
};

/**
 * 渲染字段栅格：按容器宽度自动排为 1 到 columns 列。
 *
 * @param props 最多列数与字段节点
 * @returns 字段栅格
 */
export function FieldGrid({ columns = 2, className, children }: FieldGridProps) {
  return <div className={cx("sk-grid", className)} data-columns={columns}>{children}</div>;
}
