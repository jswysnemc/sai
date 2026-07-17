import { Eye, EyeOff } from "lucide-react";
import { useState } from "react";
import "./password-field.css";

type PasswordFieldProps = {
  value: string;
  placeholder?: string;
  disabled?: boolean;
  onChange: (value: string) => void;
};

/**
 * 渲染可切换明文显示的密码输入框。
 *
 * @param props 密码值、状态和更新回调
 * @returns 密码输入组件
 */
export function PasswordField({ value, placeholder, disabled, onChange }: PasswordFieldProps) {
  const [visible, setVisible] = useState(false);
  return (
    <div className="ui-password-field">
      <input type={visible ? "text" : "password"} value={value} placeholder={placeholder} disabled={disabled} onChange={(event) => onChange(event.target.value)} autoComplete="off" spellCheck={false} />
      <button type="button" onClick={() => setVisible((current) => !current)} disabled={disabled} aria-label={visible ? "隐藏密码" : "显示密码"}>{visible ? <EyeOff size={15} /> : <Eye size={15} />}</button>
    </div>
  );
}
