import { Plug } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppConfig } from "../../api/contracts";
import { EditorHeader } from "./editor-layout";
import { ObjectListPanel } from "./object-list-panel";
import { PluginConfigEditor } from "./plugins/plugin-config-editor";
import { useI18n } from "../i18n/use-i18n";
import type { Locale } from "../i18n/locale";

type PluginSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染全部插件的列表和结构化配置。
 *
 * @param props 应用配置和更新回调
 * @returns 插件配置区域
 */
export function PluginSettingsSection({ config, onConfigChange }: PluginSettingsSectionProps) {
  const { locale, t } = useI18n();
  const plugins = config.plugins ?? {};
  const names = Object.keys(plugins);
  const [selected, setSelected] = useState(names[0] ?? "");

  useEffect(() => {
    if (!names.includes(selected)) setSelected(names[0] ?? "");
  }, [names.join("\u0000"), selected]);

  const plugin = plugins[selected] ?? {};
  return (
    <div className="settings-objects-layout">
      <ObjectListPanel
        title={t("Plugins", "插件")}
        items={names.map((name) => ({
          id: name,
          name: pluginLabel(name, locale),
          meta: name,
          icon: <Plug size={14} />,
          marked: plugins[name]?.enabled !== false
        }))}
        selectedId={selected}
        searchPlaceholder={t("Search plugins", "搜索插件")}
        onSelect={setSelected}
      />
      <section className="settings-editor">
        <EditorHeader kicker={t("Plugin capabilities", "插件能力")} title={pluginLabel(selected, locale)} description={t("Configure switches, service endpoints, credentials, and plugin runtime parameters.", "配置开关、服务地址、凭据和插件运行参数。")} />
        <PluginConfigEditor
          config={plugin}
          onChange={(next) => onConfigChange({ ...config, plugins: { ...plugins, [selected]: next } })}
        />
      </section>
    </div>
  );
}

/**
 * 返回指定语言的插件名称。
 *
 * @param name 插件配置标识
 * @param locale 当前界面语言
 * @returns 插件显示名称
 */
function pluginLabel(name: string, locale: Locale): string {
  const labels: Record<string, [string, string]> = {
    weather: ["Weather", "天气查询"],
    web: ["Web search", "网页搜索"],
    web_images: ["Web images", "网页图片"],
    deep_research: ["Deep research", "深度研究"],
    deep_diagnose: ["Deep diagnosis", "深度诊断"],
    vision: ["Vision", "视觉理解"],
    exchange_rate: ["Exchange rates", "汇率查询"],
    xuanxue: ["I Ching", "六十四卦"],
    image_generation: ["Image generation", "图片生成"],
    print_image: ["Image output", "图片输出"],
    memes: ["Meme gallery", "表情图库"],
    knowledge_base: ["Knowledge base", "知识库"],
    archlinux: ["Arch Linux", "Arch Linux"],
    man: ["Online manuals", "在线手册"],
    moegirl: ["Moegirlpedia", "萌娘百科"],
    hash_codec: ["Hash codec", "哈希编码"],
    calculator: ["Calculator", "计算器"],
    package_advisor: ["Package advisor", "软件包建议"],
    linux_game_compatibility: ["Linux game compatibility", "Linux 游戏兼容性"],
    diagnostics: ["Runtime diagnostics", "运行诊断"],
    memory: ["Long-term memory", "长期记忆"]
  };
  const label = labels[name];
  return label ? label[locale === "zh-CN" ? 1 : 0] : name.replaceAll("_", " ");
}
