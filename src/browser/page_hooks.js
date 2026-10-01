// 【内置浏览器】【页面钩子】在隔离世界中运行，补齐无头模式缺失的原生界面：
// 单选下拉框点开时不弹原生菜单，记录选项交给面板绘制；复制时读取当前选中文本。
(() => {
  const KEY = "__saiPageHooks";
  if (globalThis[KEY]) return true;
  let pending = null;
  let target = null;

  // 1. 单选下拉框：拦截按下，阻止无法显示的原生弹出菜单
  window.addEventListener(
    "mousedown",
    (event) => {
      if (event.button !== 0) return;
      const select = event.target instanceof Element ? event.target.closest("select") : null;
      if (!select || select.multiple || select.size > 1 || select.disabled) return;
      event.preventDefault();
      select.focus();
      const rect = select.getBoundingClientRect();
      target = select;
      pending = {
        options: [...select.options].map((option) => ({
          label: option.label || option.text,
          disabled: option.disabled || !!option.parentElement?.disabled,
          group: option.parentElement instanceof HTMLOptGroupElement ? option.parentElement.label : "",
        })),
        selected: select.selectedIndex,
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
      };
    },
    true,
  );

  globalThis[KEY] = {
    // 2. 取走待显示的下拉菜单，同一次点击只交出一次
    takeSelect() {
      const value = pending;
      pending = null;
      return value;
    },
    // 3. 应用面板中的选择，按真实用户操作派发 input 与 change
    applySelect(index) {
      if (!target || !target.isConnected) throw new Error("the select element is no longer on the page");
      if (index < 0 || index >= target.options.length) throw new Error("option index out of range");
      if (target.selectedIndex !== index) {
        target.selectedIndex = index;
        target.dispatchEvent(new Event("input", { bubbles: true }));
        target.dispatchEvent(new Event("change", { bubbles: true }));
      }
      return true;
    },
    // 4. 读取选中文本：输入框读选区，其余读页面选区
    selection() {
      const active = document.activeElement;
      if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) {
        if (active instanceof HTMLInputElement && active.type === "password") return "";
        const start = active.selectionStart ?? 0;
        const end = active.selectionEnd ?? 0;
        return active.value.slice(Math.min(start, end), Math.max(start, end));
      }
      return String(getSelection() ?? "");
    },
  };
  return true;
})()
