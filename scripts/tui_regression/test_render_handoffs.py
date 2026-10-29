"""【终端】【区域交接回归】提问和错误事件不能覆盖或遗留输入区域。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession


class RenderHandoffTests(unittest.TestCase):
    """使用本地模型复现独占面板和异步错误输出。"""

    def test_inline_question_replaces_input_without_empty_gap(self):
        """无参数；短会话的提问承接正文，移除旧输入框，缩放后不重新贴底。"""
        arguments = {"questions": [{"header": "Plan review", "question": "Review the full plan",
            "options": [{"label": "Approve and implement", "description": "Start implementation"},
                        {"label": "Keep planning", "description": "Revise the plan"}]}]}
        with ActivityModel("ask_question", arguments) as model:
            model.advance.set()
            config = model.config()
            config["display"]["fullscreen"] = False
            with TerminalSession(config, columns=100, rows=48,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Review plan\r")
                terminal.wait_for(lambda: "Space toggle" in terminal.text())
                for cols, rows in [(100, 48), (80, 52)]:
                    terminal.resize(cols, rows)
                    terminal.pump(.3)
                    text = terminal.text()
                    self.assertNotIn("Shift+Tab mode", text)
                    self.assertNotIn("Add a follow-up", text)
                    title = next(i for i, row in enumerate(terminal.screen.display) if "Plan review" in row)
                    self.assertLess(title, 24, text)
                terminal.send(b"\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())

    def test_jev_failure_uses_render_events_not_stderr(self):
        """无参数；本地连接失败通过转写区显示，不直接输出内部诊断行。"""
        with ActivityModel() as model:
            config = model.config()
            config["jev"] = {"endpoint_id": "fixture-jev", "routing": {"enabled": True, "timeout_seconds": 1}}
            config["model_endpoints"] = [{"id": "fixture-jev", "kind": "jev", "name": "Local failure",
                "endpoint": "http://127.0.0.1:1/systemone", "model": "jev-latest", "api_key": "fixture"}]
            with TerminalSession(config, arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect routing failure\r")
                terminal.wait_for(model.ready.is_set, timeout=15)
                self.assertNotIn("【Jev路由】【请求前预选】".encode(), terminal.raw)
                self.assertIn("Jev could not pick tools", terminal.text())
                terminal.send(b"PRESERVED-DRAFT")
                terminal.wait_for(lambda: "PRESERVED-DRAFT" in terminal.text())
