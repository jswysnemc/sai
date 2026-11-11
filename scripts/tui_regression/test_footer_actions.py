"""【终端】【底栏操作回归】验证鼠标入口、选择器和草稿恢复。"""

import json
import unittest

from terminal import TerminalSession
from test_fullscreen import click


def click_text(terminal, text, last=False):
    """参数为终端、可见文本及匹配方向；点击对应文字，返回无。"""
    from wcwidth import wcswidth
    rows = list(enumerate(terminal.text().splitlines()))
    if last:
        rows.reverse()
    for row, line in rows:
        if text in line:
            click(terminal, wcswidth(line[:line.index(text)]) + 1, row)
            terminal.pump(.2)
            return
    raise AssertionError(f"missing {text!r}:\n{terminal.text()}")


class FooterActionTests(unittest.TestCase):
    """【终端】【底栏交互】通过真实鼠标协议验证命令入口。"""

    def test_model_click_cancel_preserves_draft_and_cursor(self):
        """参数无；两种视图关闭模型面板后保留草稿与中间光标，返回无。"""
        for view in ("inline", "fullscreen"):
            with self.subTest(view=view), TerminalSession(
                columns=100, rows=30, arguments=["--view", view, "--lang", "en-US"]
            ) as terminal:
                terminal.send(b"draft")
                terminal.pump(.2)
                terminal.send(b"\x1b[D\x1b[D")
                terminal.pump(.2)
                click_text(terminal, "test  auto", last=True)
                terminal.wait_for(lambda: "Model selection" in terminal.text())
                click_text(terminal, "Cancel")
                terminal.wait_for(lambda: "draft" in terminal.text() and "Model selection" not in terminal.text())
                terminal.send(b"X")
                terminal.wait_for(lambda: "draXft" in terminal.text())

    def test_capture_survives_pager_and_resume_and_restores_on_exit(self):
        """参数无；检查真实捕获启停协议，浏览器与恢复面板返回后保留鼠标，退出时恢复终端。"""
        with TerminalSession(columns=100, rows=30, arguments=["--view", "inline", "--lang", "en-US"]) as terminal:
            terminal.wait_for(lambda: "Shift+Tab mode" in terminal.text())
            mark = len(terminal.raw)
            terminal.send(b"\x1b[5~")
            terminal.wait_for(lambda: b"\x1b[?1049h" in bytes(terminal.raw[mark:]))
            terminal.wait_for(lambda: "transcript all" in terminal.text())
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: b"\x1b[?1049l" in bytes(terminal.raw[mark:]))
            raw = bytes(terminal.raw)
            self.assertGreater(raw.rfind(b"\x1b[?1000h"), raw.rfind(b"\x1b[?1000l"))
            # 1. pyte 不还原备用屏之前的像素，验证捕获后用现有刷新键恢复模拟画面
            terminal.send(b"\x0c")
            terminal.wait_for(lambda: "Shift+Tab mode" in terminal.text())
            click_text(terminal, "test  auto", last=True)
            terminal.wait_for(lambda: "Model selection" in terminal.text())
            click_text(terminal, "Cancel")
            terminal.wait_for(lambda: "Model selection" not in terminal.text())
            terminal.send(b"/resume")
            terminal.pump(.2)
            terminal.send(b"\r")
            terminal.wait_for(lambda: "Resume session" in terminal.text())
            click_text(terminal, "Scope", last=True)
            terminal.wait_for(lambda: "All workspaces" in terminal.text())
            click_text(terminal, "Cancel", last=True)
            terminal.wait_for(lambda: "Resume session" not in terminal.text())
            raw = bytes(terminal.raw)
            self.assertGreater(raw.rfind(b"\x1b[?1000h"), raw.rfind(b"\x1b[?1000l"))
            terminal.send(b"\x03")
            terminal.pump(.2)
            terminal.send(b"\x03")
            terminal.pump(.4)
            raw = bytes(terminal.raw)
            self.assertGreater(raw.rfind(b"\x1b[?1000l"), raw.rfind(b"\x1b[?1000h"))

    def test_model_picker_selects_thinking_and_saves_with_mouse(self):
        """参数无；鼠标选择思考等级后保存到配置，返回无。"""
        with TerminalSession(columns=100, rows=30, arguments=["--lang", "en-US"]) as terminal:
            click_text(terminal, "test  auto", last=True)
            terminal.wait_for(lambda: "Model selection" in terminal.text())
            click_text(terminal, "high")
            click_text(terminal, "Save")
            terminal.wait_for(lambda: "Model selection" not in terminal.text())
            config = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
            self.assertEqual(config["providers"][0]["thinking_level"], "high")

    def test_thinking_list_can_select_and_cancel_with_mouse(self):
        """参数无；底栏思考入口打开通用列表并用鼠标确认，返回无。"""
        with TerminalSession(columns=100, rows=30, arguments=["--lang", "en-US"]) as terminal:
            click_text(terminal, "auto", last=True)
            terminal.wait_for(lambda: "Filter:" in terminal.text())
            click_text(terminal, "high")
            click_text(terminal, "Select")
            terminal.wait_for(lambda: "configuration reloaded" in terminal.text())
            config = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
            self.assertEqual(config["providers"][0]["thinking_level"], "high")

    def test_mode_context_and_mouse_capture_after_nested_picker(self):
        """参数无；切换模式、关闭上下文和嵌套面板后仍能点击底栏，返回无。"""
        with TerminalSession(columns=100, rows=30, arguments=["--view", "fullscreen", "--lang", "en-US"]) as terminal:
            terminal.send(b"KEEP-DRAFT")
            terminal.pump(.2)
            click_text(terminal, "yolo+code-model", last=True)
            terminal.wait_for(lambda: "audited+code-model" in terminal.text())
            click_text(terminal, "0.0%/", last=True)
            terminal.wait_for(lambda: "Esc Close" in terminal.text())
            click_text(terminal, "Esc Close")
            terminal.wait_for(lambda: "KEEP-DRAFT" in terminal.text())
            click_text(terminal, "test  auto", last=True)
            terminal.wait_for(lambda: "Model selection" in terminal.text())
            click_text(terminal, "Subagents")
            terminal.wait_for(lambda: "Subagent models & thinking" in terminal.text())
            click_text(terminal, "Shared defaults", last=True)
            terminal.wait_for(lambda: "Subagent settings" in terminal.text())
            click_text(terminal, "Cancel")
            terminal.wait_for(lambda: "Subagent models & thinking" in terminal.text())
            click_text(terminal, "Back")
            terminal.wait_for(lambda: "Model selection" in terminal.text())
            click_text(terminal, "Cancel")
            terminal.wait_for(lambda: "KEEP-DRAFT" in terminal.text())
            click_text(terminal, "auto", last=True)
            terminal.wait_for(lambda: "Filter:" in terminal.text())
            click_text(terminal, "Cancel")
            terminal.wait_for(lambda: "KEEP-DRAFT" in terminal.text())

    def test_model_resize_and_wheel_use_visible_rows(self):
        """参数无；中文模型、长列表、窄屏页签和短屏滚轮均使用当前绘制坐标，返回无。"""
        config = {"active_provider": "test", "providers": [{
            "id": "test", "display_name": "供应商", "base_url": "http://127.0.0.1:1/v1",
            "api_key": "local-fixture", "models": [f"model-{index:02}" for index in range(30)],
            "default_model": "model-00"}]}
        with TerminalSession(config, columns=100, rows=30, arguments=["--lang", "en-US"]) as terminal:
            click_text(terminal, "model-00  auto", last=True)
            terminal.wait_for(lambda: "Model selection" in terminal.text())
            terminal.resize(42, 12)
            terminal.wait_for(lambda: "[Save]" in terminal.text() and "供应商 / model-00" in terminal.text())
            for _ in range(12):
                click(terminal, 4, 6, kind=65)
                terminal.pump(.03)
            terminal.wait_for(lambda: "供应商 / model-12" in terminal.text())
            click_text(terminal, "供应商 / model-14")
            click_text(terminal, "Thinking")
            terminal.wait_for(lambda: "medium" in terminal.text() and "供应商 /" not in terminal.text())
            click_text(terminal, "high")
            terminal.resize(100, 30)
            terminal.wait_for(lambda: "供应商 / model-14" in terminal.text())
            click_text(terminal, "Save")
            terminal.wait_for(lambda: "Model selection" not in terminal.text())
            config = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
            self.assertEqual(config["providers"][0]["default_model"], "model-14")
            self.assertEqual(config["providers"][0]["thinking_level"], "high")

    def test_streaming_model_click_does_not_block_or_replace_draft(self):
        """参数无；执行期间模型入口沿用禁用提示，不打开阻塞面板或丢失草稿，返回无。"""
        from mock_fragmented_model import FragmentedModel
        with FragmentedModel("MOUSE-STREAM") as model, TerminalSession(
            model.config(), columns=100, rows=32, arguments=["--view", "fullscreen", "--lang", "en-US"]
        ) as terminal:
            terminal.send(b"Inspect mouse")
            terminal.pump(.2)
            terminal.send(b"\r")
            terminal.wait_for(lambda: "MOUSE-STREAM" in terminal.text())
            terminal.send(b"KEEP-QUEUED")
            terminal.pump(.2)
            rows = terminal.text().splitlines()
            row = next(index for index, line in enumerate(rows) if "yolo+code-model" in line and "auto" in line)
            column = rows[row].index("auto") - 3
            click(terminal, column, row)
            terminal.wait_for(lambda: "/model is available after this turn" in terminal.text())
            self.assertNotIn("Model selection", terminal.text())
            self.assertIn("KEEP-QUEUED", terminal.text())
            model.release.set()
            terminal.wait_for(lambda: "STREAM-DONE" in terminal.text())
