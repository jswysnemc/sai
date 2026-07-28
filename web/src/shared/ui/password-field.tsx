import { Eye, EyeOff } from "lucide-react";
import { useState } from "react";
import "./password-field.css";
import { useI18n } from "../../features/i18n/use-i18n";

type PasswordFieldProps = {
  value: string;
  placeholder?: string;
  disabled?: boolean;
  onReveal?: () => Promise<string>;
  onChange: (value: string) => void;
};

/**
 * 渲染可切换明文显示的密码输入框。
 *
 * @param props 密码值、状态和更新回调
 * @returns 密码输入组件
 */
export function PasswordField({
  value,
  placeholder,
  disabled,
  onReveal,
  onChange
}: PasswordFieldProps) {
  const { t } = useI18n();
  const [visible, setVisible] = useState(false);
  const [revealing, setRevealing] = useState(false);
  const [revealedValue, setRevealedValue] = useState<string | null>(null);

  /**
   * 切换密码可见状态，需要时先从服务端读取真实值。
   *
   * @returns 切换完成后返回
   */
  const toggleVisibility = async (): Promise<void> => {
    // 1. 隐藏时立即清除组件内暂存的真实值
    if (visible) {
      setVisible(false);
      setRevealedValue(null);
      return;
    }
    // 2. 普通密码直接切换输入类型
    if (!onReveal) {
      setVisible(true);
      return;
    }
    // 3. 脱敏密码在读取成功后再显示，避免占位符短暂闪现
    setRevealing(true);
    try {
      const secret = await onReveal();
      setRevealedValue(secret);
      setVisible(true);
    } catch {
      setVisible(false);
      setRevealedValue(null);
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
    setRevealedValue(nextValue);
    onChange(nextValue);
  };

  return (
    <div className="ui-password-field">
      <input
        type={visible ? "text" : "password"}
        value={revealedValue ?? value}
        placeholder={placeholder}
        disabled={disabled || revealing}
        onChange={(event) => updateValue(event.target.value)}
        autoComplete="off"
        spellCheck={false}
      />
      <button
        type="button"
        onClick={() => void toggleVisibility()}
        disabled={disabled || revealing}
        aria-busy={revealing}
        aria-label={revealing
          ? t("Reading password", "正在读取密码")
          : visible
            ? t("Hide password", "隐藏密码")
            : t("Show password", "显示密码")}
      >
        {visible ? <EyeOff size={15} /> : <Eye size={15} />}
      </button>
    </div>
  );
}
