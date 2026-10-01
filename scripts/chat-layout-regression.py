"""【Web 布局】【回归验证】检查输入框删除高度和消息导航条避让。"""

from importlib import import_module
import json

fixture = import_module("provider-autofill-regression")
browser = fixture.browser
request = fixture.request


def editor_state(session):
    """session 为浏览器会话；返回输入区高度与 DOM，供失败诊断。"""
    return browser(session, "eval", """(() => {
      const editor = document.querySelector('.composer-editor');
      return { height: editor.getBoundingClientRect().height, html: editor.innerHTML, text: editor.textContent };
    })()""")


def verify_composer(session):
    """session 为浏览器会话；验证逐字删除、多行与清空后的高度，返回无。"""
    browser(session, "wait", "--fn", "document.querySelector('.composer-editor')?.getAttribute('contenteditable')==='true'")
    browser(session, "click", ".composer-editor")
    baseline = editor_state(session)["height"]
    for key in ["a", "Shift+Enter", "b", "Shift+Enter", "c"]:
        browser(session, "press", key)
    assert editor_state(session)["height"] > baseline + 10, "多行文本应增高"
    previous_height = editor_state(session)["height"]
    for _ in range(6):
        browser(session, "press", "Backspace")
        state = editor_state(session)
        assert state["height"] <= previous_height + 1, f"删除文字反而增高: {state}"
        previous_height = state["height"]
    state = editor_state(session)
    assert abs(state["height"] - baseline) < 1, f"删除后输入框没有恢复高度: {state}"
    for _ in range(3):
        browser(session, "press", "x")
        browser(session, "press", "Backspace")
        state = editor_state(session)
        assert abs(state["height"] - baseline) < 1, f"反复删除导致高度增长: {state}"
    # 1. 【聊天输入】【空行与撤销】真实输入的空行保留，撤销与重做仍恢复原文
    browser(session, "press", "Shift+Enter")
    browser(session, "press", "Shift+Enter")
    assert editor_state(session)["text"] == "\n\n"
    browser(session, "press", "Backspace")
    assert editor_state(session)["text"] == "\n"
    browser(session, "press", "Control+z")
    assert editor_state(session)["text"] == "\n\n"
    browser(session, "press", "Control+Shift+z")
    assert editor_state(session)["text"] == "\n"
    browser(session, "press", "Control+a")
    browser(session, "press", "Backspace")
    assert abs(editor_state(session)["height"] - baseline) < 1


def timeline():
    """返回多轮 Markdown 测试响应，不调用真实模型。"""
    return {"turns": [{
        "turn_id": f"layout-{index}", "seq": index, "status": "completed", "automatic": False,
        "user": {"timestamp": "2026-10-02T00:00:00Z", "content": f"Layout question {index}"},
        "assistant": {"timestamp": "2026-10-02T00:00:01Z", "content":
                      "## Layout fixture\n\n" + "Paragraph with **Markdown** and `inline code`.\n\n" * 12},
        "tools": []
    } for index in range(3)]}


def verify_rail(session):
    """session 为浏览器会话；检查桌面、窄面板和移动端正文不与导航条重叠，返回无。"""
    visible_count = 0
    for width in [1000, 1050, 1200, 1440, 768, 390]:
        browser(session, "set", "viewport", str(width), "800")
        browser(session, "wait", "--fn", "!!document.querySelector('.markdown-body')")
        state = browser(session, "eval", """(() => {
          const rail = document.querySelector('.message-overview-rail');
          const body = document.querySelector('.assistant-message .markdown-body');
          const style = rail && getComputedStyle(rail);
          return {visible: !!rail && style.display !== 'none' && style.visibility !== 'hidden',
            railRight: rail?.getBoundingClientRect().right,
            contentLeft: body.getBoundingClientRect().left,
            overflow: document.documentElement.scrollWidth > innerWidth};
        })()""")
        assert not state["overflow"], f"页面出现横向溢出: {width}, {state}"
        if state["visible"]:
            visible_count += 1
            assert state["contentLeft"] >= state["railRight"] + 7, f"正文未避让导航条: {width}, {state}"
    assert visible_count > 0, "测试必须覆盖导航条可见的桌面布局"


def verify(origin, session):
    """origin 为隔离服务，session 为浏览器会话；返回布局验证结果。"""
    fixture.seed(origin)
    chat = request(origin, "/api/sessions", "POST", {"title": "Layout fixture"})
    request(origin, f"/api/sessions/{chat['id']}/switch", "POST")
    browser(session, "open", origin)
    verify_composer(session)
    browser(session, "network", "route", "**/api/sessions/*/timeline*", "--body", json.dumps(timeline()))
    browser(session, "reload")
    verify_rail(session)
    verify_composer(session)
    return {"composer_delete_height": True, "navigation_clearance": True, "responsive_layout": True}


if __name__ == "__main__":
    fixture.main(verify)
