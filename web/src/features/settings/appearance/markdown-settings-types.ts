import type {
  MarkdownCodeBlockStylePreferences,
  MarkdownStylePreferences,
  MarkdownStylePreset,
  MarkdownTableStylePreferences
} from "../../markdown/markdown-style-preferences";

export type MarkdownStyleSettingsProps = {
  preferences: MarkdownStylePreferences;
  onPresetChange: (preset: MarkdownStylePreset) => void;
  onTableChange: (patch: Partial<MarkdownTableStylePreferences>) => void;
  onCodeBlockChange: (patch: Partial<MarkdownCodeBlockStylePreferences>) => void;
  onReset: () => void;
};
