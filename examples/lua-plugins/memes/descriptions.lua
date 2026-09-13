return sai.json.decode([=[
{
  "Search the current persona's meme library by scene, mood, tags, or visible content. Use before showing a meme unless the user provided a specific meme id.": "按场景、情绪、标签或画面内容搜索当前人格表情库。除非用户给了具体表情 id，否则发表情前先调用。",
  "Scene, mood, visible content, or user intent.": "场景、情绪、画面内容或用户意图。",
  "Optional preferred tags.": "可选偏好标签。",
  "Optional meme library override.": "可选表情库覆盖。",
  "Maximum number of candidates, default 6.": "候选数量上限，默认 6。",
  "Render a meme in the terminal with terminal image protocols or an ANSI fallback. GIFs are shown as static previews unless animation is explicitly allowed in config.": "发送表情包并使用终端图片协议或 ANSI 降级渲染。GIF 默认显示静态预览，除非配置显式允许动画。",
  "Meme sha256 id.": "表情 sha256 id。",
  "Optional terminal size, e.g. 40x15.": "可选终端显示尺寸，例如 40x15。",
  "Optional output width in terminal cells.": "可选终端单元格输出宽度。",
  "Optional output height in terminal cells.": "可选终端单元格输出高度。",
  "Get the most recent meme automatically sent for the current persona/library.": "查询当前人格/表情库最近一次自动发送的表情。",
  "Add a local image to the current persona's writable meme library. If metadata is not supplied, the tool asks the configured vision model to generate it from the image.": "把本地图片加入当前人格的可写表情库。若未提供元数据，工具会调用配置的识图模型根据图片生成。",
  "Local image path.": "本地图片路径。",
  "Chinese display name.": "中文显示名。",
  "English display name.": "英文显示名。",
  "Visible content description.": "图片可见内容描述。",
  "When to use this meme.": "什么时候使用该表情。",
  "When not to use this meme.": "什么场景不要使用。",
  "Search tags.": "检索标签。",
  "Update meme index metadata in the writable overlay for the current library.": "更新当前表情库可写覆盖层中的表情元数据。",
  "Enable or disable this meme.": "启用或禁用该表情。",
  "Delete a user meme or disable a built-in meme in the current library.": "删除用户表情，或在当前表情库中禁用内置表情。",
  "Permanently remove user image instead of moving it to trash.": "永久删除用户图片，而不是移入回收站。"
}
]=])
