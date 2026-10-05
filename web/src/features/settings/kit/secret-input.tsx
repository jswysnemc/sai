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
  /** 需要先点编辑才挂载输入框时开启；设置页默认直接显示掩码 */
  requireExplicitEdit?: boolean;
};

/**
 * 渲染设置页密钥输入。
 *
 * 已保存的值直接以圆点掩码显示，输入即替换；输入框关闭自动填充，
 * 避免浏览器把设置项识别为登录密码。
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
  ariaLabel,
  requireExplicitEdit = false
}: SkSecretInputProps) {
  const { t } = useI18n();
  const field = useFieldContext();
  const saved = Boolean(secretSentinel) && value === secretSentinel;
  return (
    <div className="sk-secret">
      <PasswordField
        requireExplicitEdit={requireExplicitEdit}
        id={field?.controlId}
        ariaLabel={ariaLabel ?? field?.labelText}
        value={saved ? "" : value}
        placeholder={saved ? "••••••••••••" : placeholder}
        disabled={disabled}
        savedValueHint={saved ? t("Saved", "已保存") : undefined}
        onClearSavedValue={saved ? () => onChange("") : undefined}
        onReveal={onReveal}
        onChange={onChange}
      />
    </div>
  );
}
