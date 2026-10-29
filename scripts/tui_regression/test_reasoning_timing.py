"""【思考计时】【终端回归】等待首包和工具执行不进入单段思考耗时。"""

import json
import re
import threading
import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession


class ReasoningTimingTests(unittest.TestCase):
    """在内联与全屏界面通过真实流式入口检查思考计时。"""

    def test_thought_excludes_initial_wait_and_following_command(self):
        """无参数；首包、参数传输与工具执行各等待一秒，短思考耗时不计入这三段等待。"""
        for view in ("inline", "fullscreen"):
            with self.subTest(view=view), ActivityModel() as model:
                model.reasoning_gate = threading.Event()
                model.command = "sleep 1.2; printf TIMING-COMMAND-DONE"
                arguments = json.dumps({"command": model.command})
                model.argument_chunks = (arguments[:-2], arguments[-2:])
                model.advance.set()
                with TerminalSession(model.config(), columns=100, rows=32,
                                     arguments=["--lang", "en-US", "--view", view]) as terminal:
                    terminal.send(b"Inspect thought timing")
                    terminal.wait_for(lambda: "Inspect thought timing" in terminal.text())
                    terminal.send(b"\r")
                    terminal.wait_for(lambda: "Waiting for response" in terminal.text())
                    terminal.pump(1.2)
                    model.reasoning_gate.set()
                    terminal.wait_for(model.preparing.is_set)
                    terminal.pump(1.2)
                    model.resume_preparation.set()
                    terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                    line = next(line for line in terminal.text().splitlines() if "Thought for" in line)
                    match = re.search(r"Thought for (\d+(?:\.\d+)?)s", line)
                    self.assertIsNotNone(match, line)
                    self.assertLess(float(match.group(1)), 1.0, line)
                    self.assertIn("TIMING-COMMAND-DONE", terminal.text())
