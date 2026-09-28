import type { MarkdownStylePreset } from "../../markdown/markdown-style-preferences";

type PresetOption = {
  value: MarkdownStylePreset;
  nameEn: string;
  nameZh: string;
  descriptionEn: string;
  descriptionZh: string;
};

export const PRESET_OPTIONS: readonly PresetOption[] = [
  {
    value: "default",
    nameEn: "Default",
    nameZh: "默认",
    descriptionEn: "Balanced spacing and neutral tones.",
    descriptionZh: "均衡间距与中性配色。"
  },
  {
    value: "compact",
    nameEn: "Compact",
    nameZh: "紧凑",
    descriptionEn: "Denser headings and tighter line height.",
    descriptionZh: "更小标题与更紧的行距，信息密度优先。"
  },
  {
    value: "document",
    nameEn: "Document",
    nameZh: "文档",
    descriptionEn: "Generous whitespace for long-form reading.",
    descriptionZh: "更大留白与行高，适合长文阅读。"
  },
  {
    value: "vivid",
    nameEn: "Vivid",
    nameZh: "彩色",
    descriptionEn: "Accent-colored headings, markers, and quotes.",
    descriptionZh: "标题、列表与引用带主题色视觉锚点。"
  }
];
