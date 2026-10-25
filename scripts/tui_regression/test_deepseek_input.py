"""【终端】【输入引导回归】检查简洁样式在空闲、工作和窄窗口中的输入提示。"""

import unittest

from mock_tool_model import MockToolModel
from terminal import TerminalSession
from test_fullscreen import TurnModel


class DeepseekInputTests(unittest.TestCase):
    """真实终端中的提示应说明输入动作，而不是重复权限模式列表。"""

    def prompt(self, terminal):
        """参数 terminal 为终端；返回当前带输入标记的行。"""
        return next(line.strip() for line in terminal.text().splitlines() if "→" in line)

    def test_idle_prompt_stays_actionable_after_turns(self):
        """无参数；多轮结束后仍显示输入引导，不轮换为长功能说明。"""
        with TurnModel() as model:
            config = model.config()
            config["display"]["tui_style"] = "deepseek"
            with TerminalSession(config, columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                self.assertIn("Enter a message or type /help", self.prompt(terminal))
                for name in ["ALPHA", "BRAVO"]:
                    terminal.send((name + "\r").encode())
                    terminal.wait_for(lambda: f"ANSWER-{name}-7" in terminal.text(), timeout=15)
                    terminal.wait_for(lambda: "Enter a message or type /help" in self.prompt(terminal))
                    self.assertNotIn("permission modes", self.prompt(terminal))

    def test_narrow_idle_prompt_and_working_draft(self):
        """无参数；窄窗口保留输入引导，工作时说明排队动作，真实草稿不被提示覆盖。"""
        with MockToolModel("run_command") as model:
            config = model.config()
            config["display"] = {"tui_style": "deepseek"}
            with TerminalSession(config, columns=30, rows=24,
                    arguments=["--lang", "en-US"],
                    ready=lambda terminal: "→ Enter a message" in terminal.text()) as terminal:
                self.assertIn("Enter a message", self.prompt(terminal))
                terminal.send(b"Inspect a task\r")
                terminal.wait_for(model.ready.is_set)
                terminal.wait_for(lambda: "Queue a follow-up" in self.prompt(terminal))
                terminal.send(b"keep this draft")
                terminal.wait_for(lambda: "keep this draft" in self.prompt(terminal))
                self.assertNotIn("Queue a follow-up", self.prompt(terminal))


if __name__ == "__main__":
    unittest.main()
