import { Check, Copy, Eye, EyeOff, Loader2, X } from "lucide-react";
import { useCallback, useEffect, useState, type FocusEvent } from "react";
import "./password-field.css";
import { useI18n } from "../../features/i18n/use-i18n";

/** 明文最长驻留秒数：到点自动收回，密钥不长期留在页面里 */
const REVEAL_SECONDS = 30;

/** 复制成功标记的驻留时长 */
const COPIED_HINT_DELAY = 1600;

type PasswordFieldProps = {
  value: string;
  placeholder?: string;
  disabled?: boolean;
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
 * 明文只按需读取、按秒回收：输入框失焦、外部取值被改写或倒计时结束时
 * 立即回到掩码态并丢弃已读取的真实值，避免密钥长期留在 DOM 里。
 *
 * @param props 密码值、状态和更新回调
 * @returns 密码输入组件
 */
export function PasswordField({
  value,
  placeholder,
  disabled,
  savedValueHint,
  onClearSavedValue,
  onReveal,
  onChange
}: PasswordFieldProps) {
  const { t } = useI18n();
  const [state, setState] = useState<PasswordSecretState>(MASKED_SECRET_STATE);
  const [revealing, setRevealing] = useState(false);
  /** 明文剩余显示秒数；0 表示当前没有明文 */
  const [remaining, setRemaining] = useState(0);
  const [copied, setCopied] = useState(false);
  const [sync, setSync] = useState<PasswordValueSync>({ value, emitted: null });

  // 外部取值被改写（例如切到另一个供应商）时丢弃已读取的明文：
  // 列表按 key.id 复用同一输入框，明文会跟着组件活下来并直接露给下一个供应商。
  if (sync.value !== value) {
    const external = isExternalValueChange(value, sync.value, sync.emitted);
    setSync({ value, emitted: null });
    if (external) setState(MASKED_SECRET_STATE);
  }

  /** 收起明文并丢弃已读取的真实值。 */
  const mask = useCallback(() => {
    setState(MASKED_SECRET_STATE);
    setRemaining(0);
  }, []);

  useEffect(() => {
    if (!state.visible) return;
    setRemaining(REVEAL_SECONDS);
    const startedAt = Date.now();
    const timer = window.setInterval(() => {
      const left = REVEAL_SECONDS - Math.floor((Date.now() - startedAt) / 1000);
      if (left > 0) {
        setRemaining(left);
        return;
      }
      mask();
    }, 1000);
    return () => window.clearInterval(timer);
  }, [state.visible, mask]);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), COPIED_HINT_DELAY);
    return () => window.clearTimeout(timer);
  }, [copied]);

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

  /**
   * 焦点离开整个字段时收回明文。
   *
   * @param event 失焦事件
   * @returns 无返回值
   */
  const maskOnBlur = (event: FocusEvent<HTMLInputElement>): void => {
    // 焦点移到框内的复制按钮时保留明文，否则复制到的会是空值
    const next = event.relatedTarget;
    if (next instanceof HTMLElement && next.closest(".ui-password-field")) return;
    mask();
  };

  return (
    <div className="ui-password-field">
      <input
        type={state.visible ? "text" : "password"}
        value={displayed}
        placeholder={placeholder}
        disabled={disabled || revealing}
        onChange={(event) => updateValue(event.target.value)}
        // 失焦立即收回：明文不该留在屏幕上等人来关
        onBlur={maskOnBlur}
        autoComplete="off"
        spellCheck={false}
      />
      {savedValueHint && (
        <span className="ui-password-field-saved">
          {savedValueHint}
          {onClearSavedValue && (
            <button
              type="button"
              onClick={onClearSavedValue}
              aria-label={t("Clear the saved value", "清除已保存的值")}
              title={t("Clear the saved value", "清除已保存的值")}
            >
              <X size={11} />
            </button>
          )}
        </span>
      )}
      {state.visible && remaining > 0 && (
        <span className="ui-password-field-timer">
          {t(`Visible ${remaining}s`, `已显示 ${remaining}s`)}
        </span>
      )}
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
          {copied ? <Check size={15} /> : <Copy size={15} />}
        </button>
      )}
      <button
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
          ? <Loader2 size={15} className="spin" />
          : state.visible
            ? <EyeOff size={15} />
            : <Eye size={15} />}
      </button>
    </div>
  );
}
