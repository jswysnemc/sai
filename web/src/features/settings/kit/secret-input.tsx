import { PasswordField } from "../../../shared/ui/password-field";
import { useI18n } from "../../i18n/use-i18n";
import { useFieldContext } from "./field-context";
import "./inputs.css";

type SkSecretInputProps = {
  /** 当前值；等于脱敏占位符时表示服务端已保存且未修改 */
  value: string;
  /** 服务端用于表示敏感值未修改的占位符 */
  secretSentinel?: string;
  onChange: (value: string) => void;
  /** 按需从服务端读取明文 */
  onReveal?: () => Promise<string>;
  placeholder?: string;
  disabled?: boolean;
  ariaLabel?: string;
};

/**
 * 渲染设置页密钥输入。
 *
 * 默认展示已保存或未配置状态，明确点击编辑后才挂载输入框。
 * 防止浏览器将设置项识别为登录密码并把自动填充值写入草稿。
 *
 * @param props 当前值、脱敏占位符、更新与读取回调
 * @returns 密钥输入框
 */
export function SkSecretInput({
  value,
  secretSentinel = "",
  onChange,
  onReveal,
  placeholder,
  disabled,
  ariaLabel
}: SkSecretInputProps) {
  const { t } = useI18n();
  const field = useFieldContext();
  const saved = Boolean(secretSentinel) && value === secretSentinel;
  return (
    <div className="sk-secret">
      <PasswordField
        requireExplicitEdit
        id={field?.controlId}
        ariaLabel={ariaLabel ?? field?.labelText}
        value={saved ? "" : value}
        placeholder={saved ? t("Type a new value to replace it", "输入新值以替换") : placeholder}
        disabled={disabled}
        savedValueHint={saved ? t("Saved", "已保存") : undefined}
        onClearSavedValue={saved ? () => onChange("") : undefined}
        onReveal={onReveal}
        onChange={onChange}
      />
    </div>
  );
}
