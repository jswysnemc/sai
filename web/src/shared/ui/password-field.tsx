import { Check, Copy, Eye, EyeOff, Loader2, Pencil, Trash2 } from "./icons";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "./button/button";
import "./password-field.css";
import { useI18n } from "../../features/i18n/use-i18n";

/** 复制成功标记的驻留时长 */
const COPIED_HINT_DELAY = 1600;

type PasswordFieldProps = {
  value: string;
  /** 输入框的稳定标识，用于和外部标签关联 */
  id?: string;
  /** 无法使用可见标签时提供的辅助技术名称 */
  ariaLabel?: string;
  placeholder?: string;
  disabled?: boolean;
  /** 设置密钥需先主动编辑，默认不向浏览器暴露密码输入框 */
  requireExplicitEdit?: boolean;
  /** 已保存敏感值的标记文案，非空时在框内显示以区分「已保存」与「未设置」 */
  savedValueHint?: string;
  /** 清除已保存敏感值的回调，提供时标记上带一个清除按钮 */
  onClearSavedValue?: () => void;
  onReveal?: () => Promise<string>;
  onChange: (value: string) => void;
};

type PasswordSecretState = {
  visible: boolean;
  /** 服务端读取到的真实值；仅明文显示期间存在 */
  revealedValue: string | null;
};

/** 用于识别取值变化的同步信息 */
type PasswordValueSync = {
  /** 上一次同步过的外部取值 */
  value: string;
  /** 本组件自己向上提交过的取值 */
  emitted: string | null;
};

/** 掩码态：既不显示明文，也不保留已读取的真实值 */
const MASKED_SECRET_STATE: PasswordSecretState = { visible: false, revealedValue: null };

/**
 * 判断取值变化是否来自组件外部。
 *
 * 用户在明文态继续输入时，父级会把刚输入的内容原样回传，
 * 这时收起明文会让输入框在打字过程中跳回掩码态。
 *
 * @param value 本次渲染的外部取值
 * @param snapshot 上次渲染的外部取值
 * @param emitted 本组件自己向上提交过的取值
 * @returns 取值由外部改写时返回 true
 */
export function isExternalValueChange(
  value: string,
  snapshot: string,
  emitted: string | null
): boolean {
  return snapshot !== value && emitted !== value;
}

/**
 * 渲染可切换明文显示的密码输入框。
 *
 * 明文由眼睛按钮切换。外部取值被改写时回到掩码态并丢弃已读取的真实值，
 * 避免切到下一条记录时仍显示上一条密钥。
 *
 * @param props 密码值、状态和更新回调
 * @returns 密码输入组件
 */
export function PasswordField({
  value,
  id,
  ariaLabel,
  placeholder,
  disabled,
  requireExplicitEdit = false,
  savedValueHint,
  onClearSavedValue,
  onReveal,
  onChange
}: PasswordFieldProps) {
  const { t } = useI18n();
  const [state, setState] = useState<PasswordSecretState>(MASKED_SECRET_STATE);
  const [revealing, setRevealing] = useState(false);
  const [copied, setCopied] = useState(false);
  const [sync, setSync] = useState<PasswordValueSync>({ value, emitted: null });
  const [editing, setEditing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const editable = !requireExplicitEdit || editing;

  // 外部取值被改写（例如切到另一个供应商）时丢弃已读取的明文：
  // 列表按 key.id 复用同一输入框，明文会跟着组件活下来并直接露给下一个供应商。
  if (sync.value !== value) {
    const external = isExternalValueChange(value, sync.value, sync.emitted);
    setSync({ value, emitted: null });
    if (external) {
      setState(MASKED_SECRET_STATE);
      setEditing(false);
    }
  }

  /** 收起明文并丢弃已读取的真实值。 */
  const mask = useCallback(() => {
    setState(MASKED_SECRET_STATE);
  }, []);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), COPIED_HINT_DELAY);
    return () => window.clearTimeout(timer);
  }, [copied]);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  /**
   * 【设置密钥】【主动编辑】切换输入状态，进入时清除仅供查看的明文。
   * @returns 无；不改变配置草稿，实际输入才触发更新
   */
  const toggleEditing = (): void => {
    mask();
    setEditing((current) => !current);
  };

  /**
   * 切换密码可见状态，需要时先从服务端读取真实值。
   *
   * @returns 切换完成后返回
   */
  const toggleVisibility = async (): Promise<void> => {
    // 1. 隐藏时立即清除组件内暂存的真实值
    if (state.visible) {
      mask();
      return;
    }
    // 2. 普通密码直接切换输入类型
    if (!onReveal) {
      setState({ visible: true, revealedValue: null });
      return;
    }
    // 3. 脱敏密码在读取成功后再显示，避免占位符短暂闪现
    setRevealing(true);
    try {
      const secret = await onReveal();
      setState({ visible: true, revealedValue: secret });
    } catch {
      mask();
    } finally {
      setRevealing(false);
    }
  };

  /**
   * 更新用户编辑后的密码值。
   *
   * @param nextValue 新密码值
   * @returns 无返回值
   */
  const updateValue = (nextValue: string): void => {
    if (!editable) return;
    // 自己发出的编辑会被父级原样回传，不能被当成"换了供应商"而收起明文
    setSync((current) => ({ ...current, emitted: nextValue }));
    setState((current) => (current.visible
      ? { visible: true, revealedValue: nextValue }
      : MASKED_SECRET_STATE));
    onChange(nextValue);
  };

  const displayed = state.revealedValue ?? value;
  // 脱敏占位符背后没有可读的明文，必须先在服务端读取一次才能复制
  const copyable = state.visible ? displayed : (onReveal ? "" : value);

  /**
   * 复制当前可见的取值。
   *
   * @returns 无返回值
   */
  const copyDisplayed = (): void => {
    if (!copyable || !navigator.clipboard) return;
    void navigator.clipboard.writeText(copyable).then(() => setCopied(true));
  };

  return (
    <div className="ui-password-field">
      {editable ? <input
        ref={inputRef}
        id={id}
        aria-label={ariaLabel}
        type={state.visible ? "text" : "password"}
        value={displayed}
        placeholder={placeholder}
        disabled={disabled || revealing}
        onChange={(event) => updateValue(event.target.value)}
        autoComplete="new-password"
        data-lpignore="true"
        data-1p-ignore="true"
        spellCheck={false}
      /> : <span
        id={id}
        className="ui-password-field-value"
        aria-label={ariaLabel}
        tabIndex={0}
      >{state.visible ? displayed : value || savedValueHint ? "••••••••••••" : t("Not configured", "未设置")}</span>}
      {requireExplicitEdit && <Button
        variant="ghost"
        size="icon"
        onClick={toggleEditing}
        disabled={disabled || revealing}
        aria-label={editing ? t("Finish editing", "完成编辑") : t("Edit secret", "编辑密钥")}
        title={editing ? t("Finish editing", "完成编辑") : t("Edit secret", "编辑密钥")}
      >{editing ? <Check size={16} /> : <Pencil size={16} />}</Button>}
      {copyable.length > 0 && (
        <button
          type="button"
          onClick={copyDisplayed}
          // 阻止焦点转移：否则点击复制会先触发失焦收回，复制到的就是空值
          onMouseDown={(event) => event.preventDefault()}
          disabled={disabled}
          aria-label={copied ? t("Copied", "已复制") : t("Copy password", "复制密码")}
          title={copied ? t("Copied", "已复制") : t("Copy password", "复制密码")}
        >
          {copied ? <Check size={16} /> : <Copy size={16} />}
        </button>
      )}
      {(!savedValueHint || onReveal) && <button
        type="button"
        onClick={() => void toggleVisibility()}
        onMouseDown={(event) => event.preventDefault()}
        disabled={disabled || revealing}
        aria-busy={revealing}
        aria-label={revealing
          ? t("Reading password", "正在读取密码")
          : state.visible
            ? t("Hide password", "隐藏密码")
            : t("Show password", "显示密码")}
      >
        {revealing
          ? <Loader2 size={16} className="spin" />
          : state.visible
            ? <EyeOff size={16} />
            : <Eye size={16} />}
      </button>}
      {savedValueHint && onClearSavedValue && (
        <button
          type="button"
          className="ui-password-field-clear"
          onClick={onClearSavedValue}
          onMouseDown={(event) => event.preventDefault()}
          disabled={disabled || revealing}
          aria-label={t("Clear the saved value", "清除已保存的值")}
          title={t("Clear the saved value", "清除已保存的值")}
        >
          <Trash2 size={16} />
        </button>
      )}
    </div>
  );
}
