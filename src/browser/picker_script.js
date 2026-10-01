// 【内置浏览器】【元素选择】在隔离世界中运行：高亮鼠标下的元素并显示尺寸与样式，
// 点击后返回元素信息，Esc 或再次调用取消脚本时返回 cancelled。
// 覆盖层挂在封闭 Shadow DOM 中，页面脚本读不到；选择期间页面收不到点击与按下事件。
(() => {
  const KEY = "__saiElementPicker";
  globalThis[KEY]?.cancel?.();
  const MAX_TEXT = 4000;
  const MAX_HTML = 6000;
  const MAX_ATTR = 500;
  const LABELS = __LABELS__;

  const truncate = (value, max) => {
    const text = String(value ?? "").replace(/\s+/g, " ").trim();
    return text.length > max ? `${text.slice(0, max)}…` : text;
  };
  const hex = (value) => {
    const match = /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/i.exec(value.trim());
    if (!match) return value.trim();
    const alpha = match[4] ? (match[4].endsWith("%") ? Number(match[4].slice(0, -1)) / 100 : Number(match[4])) : 1;
    if (alpha <= 0) return "transparent";
    return `#${[match[1], match[2], match[3]].map((channel) => Math.max(0, Math.min(255, Math.round(Number(channel)))).toString(16).padStart(2, "0")).join("").toUpperCase()}`;
  };
  const styleOf = (el) => {
    const style = getComputedStyle(el);
    const background = hex(style.backgroundColor);
    return {
      ...(background !== "transparent" ? { backgroundColor: background } : {}),
      color: hex(style.color),
      display: style.display,
      fontFamily: truncate(style.fontFamily, 160),
      fontSize: style.fontSize,
      fontWeight: style.fontWeight,
    };
  };
  const cssEscape = (value) => (globalThis.CSS?.escape ? CSS.escape(value) : value.replace(/[^a-zA-Z0-9_-]/g, "\\$&"));
  const selectorOf = (el) => {
    if (el.id) return `#${cssEscape(el.id)}`;
    const parts = [];
    for (let node = el; node && node.nodeType === 1 && parts.length < 6; node = node.parentElement) {
      let part = node.tagName.toLowerCase();
      if (node.id) {
        parts.unshift(`#${cssEscape(node.id)}`);
        break;
      }
      const testId = node.getAttribute("data-testid");
      if (testId) {
        parts.unshift(`${part}[data-testid="${testId.replace(/"/g, '\\"')}"]`);
        break;
      }
      const parent = node.parentElement;
      if (parent) {
        const same = [...parent.children].filter((child) => child.tagName === node.tagName);
        if (same.length > 1) part += `:nth-of-type(${same.indexOf(node) + 1})`;
      }
      parts.unshift(part);
    }
    return parts.join(" > ");
  };
  const xpathOf = (el) => {
    const parts = [];
    for (let node = el; node && node.nodeType === 1; node = node.parentElement) {
      const same = node.parentElement ? [...node.parentElement.children].filter((child) => child.tagName === node.tagName) : [node];
      parts.unshift(`${node.tagName.toLowerCase()}[${same.indexOf(node) + 1}]`);
    }
    return `/${parts.join("/")}`;
  };
  const implicitRole = (el) => {
    const tag = el.tagName.toLowerCase();
    if (tag === "a" && el.hasAttribute("href")) return "link";
    if (tag === "button") return "button";
    if (tag === "select") return "combobox";
    if (tag === "textarea") return "textbox";
    if (/^h[1-6]$/.test(tag)) return "heading";
    if (tag === "img") return "img";
    if (tag !== "input") return "";
    const type = (el.getAttribute("type") || "text").toLowerCase();
    if (["button", "submit", "reset"].includes(type)) return "button";
    if (type === "checkbox" || type === "radio") return type;
    return "textbox";
  };
  const nameOf = (el) => {
    const label = el.getAttribute("aria-label");
    if (label) return truncate(label, 200);
    const labelled = el.getAttribute("aria-labelledby");
    if (labelled) {
      const text = labelled.split(/\s+/).map((id) => document.getElementById(id)?.textContent ?? "").join(" ");
      if (text.trim()) return truncate(text, 200);
    }
    if (el.labels?.length) return truncate([...el.labels].map((item) => item.textContent).join(" "), 200);
    return truncate(el.getAttribute("alt") || el.getAttribute("title") || el.getAttribute("placeholder"), 200);
  };
  const textOf = (el) => {
    if (el instanceof HTMLInputElement && el.type.toLowerCase() === "password") return "[masked password input]";
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return truncate(el.value || el.placeholder, MAX_TEXT);
    return truncate(el.innerText || el.textContent, MAX_TEXT);
  };
  const nearbyOf = (el) => {
    const container = el.closest("li,tr,section,article,form,fieldset,p,div") || el.parentElement;
    return container && container !== el ? truncate(container.innerText || container.textContent, 600) : "";
  };
  const htmlOf = (el) => {
    const clone = el.cloneNode(true);
    clone.querySelectorAll?.("script,style,svg path").forEach((node) => node.remove());
    clone.querySelectorAll?.("input[type=password]").forEach((node) => node.setAttribute("value", ""));
    const html = clone.outerHTML || "";
    return html.length > MAX_HTML ? `${html.slice(0, MAX_HTML)}…` : html;
  };
  const attributesOf = (el) => {
    const result = {};
    for (const attr of el.attributes) {
      if (attr.name === "style" || attr.name.startsWith("on")) continue;
      if (el instanceof HTMLInputElement && el.type === "password" && attr.name === "value") continue;
      result[attr.name] = truncate(attr.value, MAX_ATTR);
      if (Object.keys(result).length >= 24) break;
    }
    return result;
  };

  // 1. 覆盖层：高亮框与信息卡，pointer-events 关闭，不影响命中检测
  const host = document.createElement("div");
  host.style.cssText = "position:fixed;inset:0;z-index:2147483647;pointer-events:none;";
  const root = host.attachShadow({ mode: "closed" });
  root.innerHTML = `<style>
    .box{position:fixed;display:none;border:2px solid #4c8dff;background:rgba(76,141,255,.14);border-radius:2px;box-sizing:border-box}
    .card{position:fixed;display:none;max-width:280px;padding:6px 8px;border-radius:6px;background:#1f2328;color:#e6edf3;
      font:12px/1.5 ui-sans-serif,system-ui,sans-serif;box-shadow:0 2px 8px rgba(0,0,0,.3)}
    .head{display:flex;gap:8px;justify-content:space-between;font-weight:600}
    .tag{color:#7ee787}.size{color:#a5b4c3;font-weight:400}
    .row{display:flex;gap:8px;justify-content:space-between;color:#a5b4c3}
    .row b{color:#e6edf3;font-weight:400;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:170px}
    .dot{display:inline-block;width:9px;height:9px;margin-right:4px;border-radius:2px;border:1px solid #555;vertical-align:-1px}
  </style><div class="box"></div><div class="card"></div>`;
  const box = root.querySelector(".box");
  const card = root.querySelector(".card");
  document.documentElement.appendChild(host);
  const previousCursor = document.documentElement.style.cursor;
  document.documentElement.style.cursor = "crosshair";

  let hovered = null;
  let settled = false;
  let finish = () => {};

  const row = (label, value, color) =>
    `<div class="row"><span>${label}</span><b>${color ? `<span class="dot" style="background:${color}"></span>` : ""}${value
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")}</b></div>`;
  const show = (el) => {
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      box.style.display = card.style.display = "none";
      return;
    }
    Object.assign(box.style, { display: "block", left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px` });
    const style = styleOf(el);
    card.innerHTML =
      `<div class="head"><span class="tag">${el.tagName.toLowerCase()}</span><span class="size">${Math.round(rect.width)}×${Math.round(rect.height)}</span></div>` +
      (style.backgroundColor ? row(LABELS.background, style.backgroundColor, style.backgroundColor) : "") +
      row(LABELS.color, style.color, style.color) +
      row(LABELS.font, truncate(`${style.fontSize} ${style.fontFamily}`, 60));
    card.style.display = "block";
    const width = card.offsetWidth || 220;
    const height = card.offsetHeight || 80;
    const below = rect.bottom + 6 + height <= innerHeight;
    const top = below ? rect.bottom + 6 : Math.max(4, rect.top - height - 6);
    const left = Math.min(Math.max(4, rect.left), Math.max(4, innerWidth - width - 4));
    Object.assign(card.style, { left: `${left}px`, top: `${top}px` });
  };
  const collect = (el) => {
    const rect = el.getBoundingClientRect();
    return {
      pageUrl: location.href,
      pageTitle: document.title,
      tagName: el.tagName.toLowerCase(),
      role: el.getAttribute("role") || implicitRole(el) || undefined,
      accessibleName: nameOf(el) || undefined,
      selector: selectorOf(el),
      xpath: xpathOf(el),
      text: textOf(el) || undefined,
      nearbyText: nearbyOf(el) || undefined,
      htmlExcerpt: htmlOf(el) || undefined,
      attributes: attributesOf(el),
      rect: { x: Math.round(rect.x), y: Math.round(rect.y), width: Math.round(rect.width), height: Math.round(rect.height) },
      style: styleOf(el),
    };
  };

  // 2. 捕获阶段拦截，页面自身的点击与按下逻辑不会触发
  const onMove = (event) => {
    if (!(event.target instanceof Element) || event.target === host) return;
    hovered = event.target;
    show(hovered);
  };
  const block = (event) => {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();
  };
  const onClick = (event) => {
    block(event);
    finish(hovered ? { status: "selected", element: collect(hovered) } : { status: "cancelled" });
  };
  const onKey = (event) => {
    if (event.key !== "Escape") return;
    block(event);
    finish({ status: "cancelled" });
  };
  const listeners = [
    ["mousemove", onMove],
    ["pointerdown", block],
    ["mousedown", block],
    ["pointerup", block],
    ["mouseup", block],
    ["click", onClick],
    ["auxclick", block],
    ["contextmenu", block],
    ["keydown", onKey],
  ];
  for (const [type, handler] of listeners) window.addEventListener(type, handler, true);

  return new Promise((resolve) => {
    finish = (result) => {
      if (settled) return;
      settled = true;
      for (const [type, handler] of listeners) window.removeEventListener(type, handler, true);
      host.remove();
      document.documentElement.style.cursor = previousCursor;
      delete globalThis[KEY];
      resolve(result);
    };
    globalThis[KEY] = { cancel: () => finish({ status: "cancelled" }) };
  });
})()
