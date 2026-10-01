// 【内置浏览器】【页面快照】遍历扁平化 DOM（含开放 Shadow DOM 与同源 iframe），
// 生成带 ref 的无障碍结构文本；ref 与元素的对应关系保存在隔离世界的 globalThis.__saiRefs 中，
// 页面脚本看不到也改不了，后续点击、输入通过 ref 找回元素。参数占位符由 Rust 侧替换。
(() => {
  const MAX_CHARS = __MAX_CHARS__;
  const INTERACTIVE_ONLY = __INTERACTIVE_ONLY__;
  const refs = new Map();
  globalThis.__saiRefs = refs;
  const lines = [];
  let length = 0;
  let counter = 0;
  let truncated = false;
  // 大于 0 时只输出可交互元素：段落正文已整体输出，子文本不再重复
  let textMuted = 0;

  const SKIP_TAGS = new Set(["SCRIPT", "STYLE", "NOSCRIPT", "TEMPLATE", "HEAD", "META", "LINK", "svg", "SVG"]);
  const INTERACTIVE_ROLES = new Set([
    "button", "link", "textbox", "searchbox", "combobox", "listbox", "checkbox", "radio", "switch",
    "slider", "spinbutton", "tab", "menuitem", "menuitemcheckbox", "menuitemradio", "option", "treeitem"
  ]);
  // 这些角色的名称已经概括了子内容，不再向下展开
  const LEAF_ROLES = new Set([
    "button", "link", "textbox", "searchbox", "checkbox", "radio", "switch", "slider", "spinbutton",
    "img", "heading", "option", "menuitem", "menuitemcheckbox", "menuitemradio", "tab", "treeitem"
  ]);
  const CONTAINER_ROLES = new Set([
    "navigation", "main", "form", "list", "table", "dialog", "alertdialog", "menu", "menubar",
    "tablist", "tree", "region", "search", "toolbar", "grid", "radiogroup", "group", "tabpanel"
  ]);
  // 无交互后代时整体压成一行文本的角色
  const COMPACT_ROLES = new Set(["listitem", "row", "cell", "columnheader", "rowheader", "paragraph", "blockquote"]);
  const INTERACTIVE_SELECTOR = [
    "a[href]", "button", "input:not([type=hidden])", "select", "textarea", "summary",
    "[contenteditable='']", "[contenteditable='true']", "[tabindex]:not([tabindex='-1'])", "[onclick]",
    ...[...INTERACTIVE_ROLES].map((role) => `[role='${role}']`)
  ].join(",");

  const clean = (value, max = 120) => {
    const text = String(value ?? "").replace(/\s+/g, " ").trim();
    return text.length > max ? `${text.slice(0, max - 1)}…` : text;
  };
  const quote = (value) => JSON.stringify(value);

  const push = (depth, text) => {
    if (truncated) return;
    const line = `${"  ".repeat(Math.min(depth, 12))}- ${text}`;
    if (length + line.length + 1 > MAX_CHARS) {
      truncated = true;
      return;
    }
    lines.push(line);
    length += line.length + 1;
  };

  const roleOf = (el) => {
    const explicit = el.getAttribute("role");
    if (explicit) return explicit.trim().split(/\s+/)[0].toLowerCase();
    switch (el.tagName) {
      case "A": return el.hasAttribute("href") ? "link" : null;
      case "BUTTON": case "SUMMARY": return "button";
      case "SELECT": return el.multiple || el.size > 1 ? "listbox" : "combobox";
      case "TEXTAREA": return "textbox";
      case "INPUT": {
        const type = (el.getAttribute("type") || "text").toLowerCase();
        if (type === "hidden") return null;
        if (["button", "submit", "reset", "image", "file", "color"].includes(type)) return "button";
        if (type === "checkbox") return "checkbox";
        if (type === "radio") return "radio";
        if (type === "range") return "slider";
        if (type === "number") return "spinbutton";
        if (type === "search") return "searchbox";
        return "textbox";
      }
      case "H1": case "H2": case "H3": case "H4": case "H5": case "H6": return "heading";
      case "IMG": return "img";
      case "NAV": return "navigation";
      case "MAIN": return "main";
      case "FORM": return "form";
      case "TABLE": return "table";
      case "TR": return "row";
      case "TH": return "columnheader";
      case "TD": return "cell";
      case "UL": case "OL": return "list";
      case "LI": return "listitem";
      case "P": return "paragraph";
      case "BLOCKQUOTE": return "blockquote";
      case "DIALOG": return "dialog";
      case "IFRAME": case "FRAME": return "iframe";
      default: break;
    }
    if (el.isContentEditable && el.hasAttribute("contenteditable")) return "textbox";
    return null;
  };

  const labelText = (el) => {
    const aria = el.getAttribute("aria-label");
    if (aria && aria.trim()) return clean(aria);
    const labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) {
      const text = labelledBy.split(/\s+/)
        .map((id) => el.ownerDocument.getElementById(id)?.innerText ?? "")
        .join(" ");
      if (clean(text)) return clean(text);
    }
    return "";
  };

  const nameOf = (el, role) => {
    const label = labelText(el);
    if (label) return label;
    const tag = el.tagName;
    if (tag === "IMG") return clean(el.getAttribute("alt") || el.getAttribute("title"));
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") {
      const type = (el.getAttribute("type") || "").toLowerCase();
      if (["button", "submit", "reset"].includes(type)) return clean(el.value || type);
      if (el.labels && el.labels.length) return clean([...el.labels].map((item) => item.innerText).join(" "));
      return clean(el.getAttribute("placeholder") || el.getAttribute("title") || el.getAttribute("name"));
    }
    let text = el.innerText;
    if (!clean(text)) {
      const img = el.querySelector("img[alt]");
      text = img?.getAttribute("alt") || el.getAttribute("title") || "";
    }
    return clean(text, role === "heading" ? 200 : 120);
  };

  const shortHref = (el) => {
    const href = el.getAttribute("href");
    if (!href || href.startsWith("javascript:")) return "";
    try {
      const url = new URL(href, el.baseURI);
      const text = url.origin === location.origin ? `${url.pathname}${url.search}${url.hash}` : url.href;
      return clean(text, 100);
    } catch (_) {
      return clean(href, 100);
    }
  };

  const stateOf = (el, role) => {
    const parts = [];
    if (role === "heading") {
      const level = el.getAttribute("aria-level") || el.tagName.match(/^H(\d)$/)?.[1];
      if (level) parts.push(`[level=${level}]`);
    }
    if (el.disabled || el.getAttribute("aria-disabled") === "true") parts.push("[disabled]");
    if (el.checked === true || el.getAttribute("aria-checked") === "true") parts.push("[checked]");
    if (el.getAttribute("aria-expanded") === "true") parts.push("[expanded]");
    if (el.getAttribute("aria-expanded") === "false") parts.push("[collapsed]");
    if (el.getAttribute("aria-selected") === "true" || (el.tagName === "OPTION" && el.selected)) parts.push("[selected]");
    if (el.required) parts.push("[required]");
    if (role === "link") {
      const href = shortHref(el);
      if (href) parts.push(`-> ${href}`);
    }
    if (el.tagName === "SELECT") {
      const options = [...el.options].slice(0, 20).map((option) => clean(option.text, 40));
      parts.push(`value=${quote(clean(el.selectedOptions[0]?.text ?? "", 60))}`);
      parts.push(`options=[${options.join(" | ")}${el.options.length > 20 ? " | …" : ""}]`);
    } else if (["textbox", "searchbox", "spinbutton", "combobox", "slider"].includes(role)) {
      const value = el.isContentEditable ? el.innerText : el.value;
      if (value) {
        const type = (el.getAttribute("type") || "").toLowerCase();
        parts.push(`value=${quote(type === "password" ? "••••" : clean(value, 80))}`);
      }
    }
    return parts.length ? ` ${parts.join(" ")}` : "";
  };

  const isHidden = (el, style) =>
    style.visibility === "hidden" || style.visibility === "collapse" || Number(style.opacity) === 0;

  // 祖先折叠、透明或被 content-visibility 跳过时都视为不可见
  const rendered = (el) => {
    if (typeof el.checkVisibility === "function") {
      return el.checkVisibility({ opacityProperty: true, visibilityProperty: true, contentVisibilityAuto: true });
    }
    return hasBox(el);
  };

  const hasBox = (el) => {
    const rect = el.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };

  const clickableGeneric = (el, style) => {
    if (el.hasAttribute("onclick")) return true;
    const tabindex = el.getAttribute("tabindex");
    if (tabindex !== null && Number(tabindex) >= 0) return true;
    if (style.cursor !== "pointer") return false;
    // 只标记指针样式的最外层元素，子元素继承的 pointer 不重复计数
    const parent = el.parentElement;
    return !parent || getComputedStyle(parent).cursor !== "pointer";
  };

  const assignRef = (el) => {
    counter += 1;
    const ref = `e${counter}`;
    refs.set(ref, el);
    return ref;
  };

  const childNodesOf = (node) => {
    if (node.shadowRoot) return [...node.shadowRoot.childNodes];
    if (node.tagName === "SLOT") {
      const assigned = node.assignedNodes({ flatten: true });
      return assigned.length ? assigned : [...node.childNodes];
    }
    return [...node.childNodes];
  };

  const walkChildren = (node, depth) => {
    for (const child of childNodesOf(node)) {
      if (truncated) return;
      if (child.nodeType === Node.TEXT_NODE) {
        if (INTERACTIVE_ONLY || textMuted > 0) continue;
        const text = clean(child.textContent, 600);
        const parent = child.parentElement;
        if (text.length > 1 && parent && rendered(parent)) push(depth, `text: ${text}`);
      } else if (child.nodeType === Node.ELEMENT_NODE) {
        walk(child, depth);
      }
    }
  };

  const walkFrame = (el, depth) => {
    const title = clean(el.getAttribute("title") || el.getAttribute("name") || el.getAttribute("src"), 80);
    let body = null;
    try {
      body = el.contentDocument?.body ?? null;
    } catch (_) {
      body = null;
    }
    if (!body) {
      if (!INTERACTIVE_ONLY) push(depth, `iframe ${quote(title)} (cross-origin, not inspected)`);
      return;
    }
    push(depth, `iframe ${quote(title)}`);
    walkChildren(body, depth + 1);
  };

  const walk = (el, depth) => {
    if (truncated || SKIP_TAGS.has(el.tagName)) return;
    if (el.getAttribute("aria-hidden") === "true") return;
    if (el.tagName === "INPUT" && (el.getAttribute("type") || "").toLowerCase() === "hidden") return;
    const style = getComputedStyle(el);
    if (style.display === "none") return;
    const role = roleOf(el);
    if (role === "iframe") {
      if (hasBox(el)) walkFrame(el, depth);
      return;
    }
    const visible = !isHidden(el, style) && (hasBox(el) || style.display === "contents");
    if (!visible) {
      // 自身不可见的元素仍可能包含绝对定位的可见子元素
      walkChildren(el, depth);
      return;
    }
    const interactive = (role && INTERACTIVE_ROLES.has(role)) || (!role && clickableGeneric(el, style));
    if (interactive) {
      const shownRole = role ?? "clickable";
      const name = nameOf(el, shownRole);
      push(depth, `${shownRole}${name ? ` ${quote(name)}` : ""} [ref=${assignRef(el)}]${stateOf(el, shownRole)}`);
      if (role && LEAF_ROLES.has(role)) return;
      if (el.tagName === "SELECT") return;
      walkChildren(el, depth + 1);
      return;
    }
    if (INTERACTIVE_ONLY || textMuted > 0) {
      walkChildren(el, depth);
      return;
    }
    // 段落内夹着链接时，正文整体一行，下面只列出链接 ref
    if (role === "paragraph" || role === "blockquote") {
      const text = clean(el.innerText, 1200);
      if (text) push(depth, `${role}: ${text}`);
      textMuted += 1;
      walkChildren(el, depth + 1);
      textMuted -= 1;
      return;
    }
    if (role === "heading" || role === "img") {
      const name = nameOf(el, role);
      if (name) push(depth, `${role} ${quote(name)}${stateOf(el, role)}`);
      if (role === "heading" && el.querySelector(INTERACTIVE_SELECTOR)) walkChildren(el, depth + 1);
      return;
    }
    if (role && COMPACT_ROLES.has(role) && !el.querySelector(INTERACTIVE_SELECTOR)) {
      const text = role === "row"
        ? [...el.children].map((cell) => clean(cell.innerText, 80)).filter(Boolean).join(" | ")
        : clean(el.innerText, 600);
      if (text) push(depth, `${role}: ${text}`);
      return;
    }
    // 单元格、含交互内容的列表项与布局表格（无表头）只承担排版，不占层级
    if (["cell", "columnheader", "rowheader", "listitem"].includes(role)
      || ((role === "table" || role === "row") && !el.getAttribute("role") && !el.closest("table")?.querySelector("th"))) {
      walkChildren(el, depth);
      return;
    }
    if (role && (CONTAINER_ROLES.has(role) || COMPACT_ROLES.has(role))) {
      const label = labelText(el);
      push(depth, `${role}${label ? ` ${quote(label)}` : ""}`);
      walkChildren(el, depth + 1);
      return;
    }
    walkChildren(el, depth);
  };

  if (document.body) walk(document.body, 0);
  const scrolling = document.scrollingElement || document.documentElement;
  return {
    title: document.title,
    url: location.href,
    text: lines.join("\n"),
    refs: counter,
    truncated,
    scrollY: Math.round(scrolling.scrollTop),
    scrollHeight: Math.round(scrolling.scrollHeight),
    viewportHeight: window.innerHeight
  };
})()
