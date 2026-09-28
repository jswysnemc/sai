import { useState } from "react";
import { SkTextArea } from "./text-input";

type SkListInputProps = {
  value: string[];
  onChange: (value: string[]) => void;
  rows?: number;
  placeholder?: string;
};

/**
 * 【Web 设置】【列表输入】以每行一项编辑字符串列表，保留输入中的空行和空格。
 * @param props 列表值、更新回调、行数与占位提示
 * @returns 多行列表输入框
 */
export function SkListInput({ value, onChange, rows = 3, placeholder }: SkListInputProps) {
  const source = JSON.stringify(value);
  const [draft, setDraft] = useState<{ source: string; text: string } | null>(null);
  const text = draft?.source === source ? draft.text : value.join("\n");

  /**
   * 保留完整文本草稿，同时把非空条目写入配置。
   * @param nextText 用户输入文本
   * @returns 无返回值
   */
  const update = (nextText: string) => {
    const next = nextText.split("\n").map((item) => item.trim()).filter(Boolean);
    setDraft({ source: JSON.stringify(next), text: nextText });
    onChange(next);
  };

  return <SkTextArea mono rows={rows} value={text} placeholder={placeholder} onChange={update} onBlur={() => setDraft(null)} />;
}
