type Translate = (en: string, zh: string) => string;

/** 工具卡头部的短标签：英文保持现有种类名，中文走界面语言。 */
const TOOL_KIND_LABELS: Record<string, readonly [string, string]> = {
  run_command: ["Shell", "命令"],
  background_command: ["Shell", "命令"],
  edit_file: ["Edit", "编辑"],
  write_file: ["Write", "写入"],
  str_replace: ["Replace", "替换"],
  read_file: ["Read", "读取"],
  grep: ["Search", "搜索"],
  glob: ["Files", "文件"],
  list_dir: ["List", "目录"],
  list_directory: ["List", "目录"],
  trash_path: ["Trash", "回收站"],
  todo: ["Todo", "待办"],
  load: ["Load", "加载"],
  request_capability: ["Request", "申请"],
  generate_image: ["Generate image", "生成图片"]
};

/**
 * 与后端 `readable_tool_name` 对齐的工具名称。
 *
 * Jev 折叠行用它代替原始标识，避免中文界面只看到 `context_status`。
 */
const TOOL_TITLES_ZH: Record<string, string> = {
  run_command: "运行命令",
  background_command: "后台命令",
  subagent: "子智能体",
  todo: "任务清单",
  cron: "定时任务",
  read_file: "读取文件",
  request_capability: "能力申请",
  write_file: "写入文件",
  str_replace: "字符串替换",
  create_goal: "创建目标",
  get_goal: "查看目标",
  update_goal: "更新目标",
  list_directory: "列目录",
  list_dir: "列目录",
  create_directory: "创建目录",
  trash_path: "移入回收站",
  find_files: "查找文件",
  glob: "查找文件",
  search_text: "搜索文本",
  grep: "搜索文本",
  get_current_directory: "当前目录",
  get_current_time: "当前时间",
  check_issue: "检查问题",
  check_os_info: "查看系统信息",
  web_search: "网页搜索",
  web_fetch: "读取网页",
  browser: "浏览器",
  search_web_images: "搜索图片",
  print_image: "显示图片",
  generate_image: "生成图片",
  search_meme: "搜索表情包",
  show_meme: "发送表情",
  add_meme: "添加表情包",
  update_meme: "更新表情包",
  delete_meme: "删除表情包",
  deep_diagnose: "输入法诊断",
  linux_input_method_diagnose: "输入法诊断",
  upload_knowledge_base_file: "导入知识库",
  upload_text_to_knowledge_base: "导入知识库",
  read_knowledge_base_file: "读取知识库",
  search_knowledge_base: "搜索知识库",
  search_knowledge_base_by_name: "按名称搜索知识库",
  edit_knowledge_base_file: "编辑知识库",
  remove_knowledge_base_file: "移除知识库",
  list_knowledge_base_files: "列出知识库",
  set_alarm: "设置闹钟",
  list_alarms: "列出闹钟",
  cancel_alarm: "取消闹钟",
  write_memory: "写入记忆",
  read_memory: "读取记忆",
  list_memory: "列出记忆",
  delete_memory: "删除记忆",
  search_evicted_context: "搜索旧上下文",
  context_status: "查看上下文块",
  compress_context: "压缩上下文块",
  search_context: "搜索上下文原文",
  restore_context: "回读上下文原文",
  aur_search_packages: "搜索 AUR",
  aur_get_package_info: "查看 AUR 包",
  aur_check_status: "查询 AUR 状态",
  archlinux_official_package_query: "查询 Arch 官方包",
  query_deepseek_status: "查询 DeepSeek 状态",
  pacman_search: "搜索软件包",
  archwiki_query: "查询 ArchWiki",
  online_man_search: "搜索在线手册",
  man_search: "搜索在线手册",
  online_man_get_page: "读取在线手册",
  man_read: "读取在线手册",
  moegirl_query: "查询萌娘百科",
  calculate: "计算",
  calculator: "计算",
  scientific_calculator: "计算",
  calculate_hash: "计算哈希",
  decode_encoded_text: "解码文本",
  exchange_rate: "汇率查询",
  get_exchange_rate: "汇率查询",
  weather: "天气查询",
  get_weather: "天气查询",
  protondb_query: "查询 ProtonDB",
  xuanxue_pick: "玄学选择",
  xuanxue_divine: "玄学占卜",
  draw_zhouyi_hexagram: "周易起卦",
  draw_tarot_card: "抽塔罗牌",
  draw_fortune_lot: "吉凶占",
  roll_dice: "掷骰子",
  load: "加载",
  review_aur_package: "审查 AUR 包",
  install_aur_package: "安装 AUR 包",
  review_pkgbuild_directory: "审查 PKGBUILD 目录",
  linux_game_compatibility: "查询 Linux 游戏兼容性",
  gather_linux_game_compatibility_signals: "收集游戏兼容性",
  register_linux_game_evidence: "登记兼容性证据",
  send_channel_image: "发送渠道图片",
  send_channel_file: "发送渠道文件",
  send_channel_video: "发送渠道视频",
  edit_file: "编辑文件",
  fcitx5_input_method_wiki_qurey: "查询 Fcitx5 Wiki"
};

/**
 * 工具卡种类名。未知工具保留把下划线换成空格后的标识。
 *
 * @param name 工具标识
 * @param backgroundTask 是否为后台任务管理操作
 * @param t 双语文本选择方法
 * @returns 当前界面语言下的种类名
 */
export function toolKindLabel(name: string, backgroundTask: boolean, t: Translate): string {
  if (name === "background_command" && backgroundTask) return t("Tasks", "任务");
  const known = TOOL_KIND_LABELS[name];
  if (known) return t(known[0], known[1]);
  const fallback = name.replaceAll("_", " ");
  return t(fallback, fallback);
}

/**
 * 是否有面向界面的工具名称，而不是只能显示原始标识。
 *
 * @param name 工具标识
 * @returns 名称表中有条目时为 true
 */
export function hasLocalizedToolTitle(name: string): boolean {
  return Object.prototype.hasOwnProperty.call(TOOL_TITLES_ZH, name);
}

/**
 * Jev 暴露名单上的工具名称。
 *
 * @param name 工具标识
 * @param t 双语文本选择方法
 * @returns 当前界面语言下的名称；未知工具退回空格分隔的标识
 */
export function toolExposureTitle(name: string, t: Translate): string {
  const zh = TOOL_TITLES_ZH[name];
  const en = englishTitle(name);
  if (!zh) return t(en, en);
  return t(en, zh);
}

/**
 * 把工具标识转成英文标题。
 *
 * @param name 工具标识
 * @returns 首字母大写的英文标题
 */
function englishTitle(name: string): string {
  return name
    .replaceAll("_", " ")
    .replace(/(^|\s)([a-z])/gu, (_match, space: string, letter: string) => `${space}${letter.toUpperCase()}`);
}
