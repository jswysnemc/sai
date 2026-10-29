"""【终端】【活动动效回归】暂停模型流和命令输出，确认时钟仍驱动节点标记。"""

import shlex
import json
import re
import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession


class ActivityLivePulseTests(unittest.TestCase):
    """【终端】【活动动效】检查真实终端中的准备、执行、思考与完成状态。"""

    def markers(self, terminal, label):
        """参数 terminal 为终端、label 为标题文本；返回一段无新内容期间的标记集合。"""
        frames = set()
        for _ in range(12):
            terminal.pump(.08)
            line = next(line for line in terminal.text().splitlines() if label in line)
            frames.add(line.lstrip()[0])
        return frames

    def tool_pulses(self, terminal, label, icon):
        """参数为终端、标题和语义图标；验证字形固定而亮度随时钟变化，返回无。"""
        start = len(terminal.raw)
        self.assertEqual(self.markers(terminal, label), {icon})
        pattern = re.escape(icon.encode()) + rb"(\x1b\[22m)?"
        styles = {bool(match.group(1)) for match in re.finditer(pattern, bytes(terminal.raw[start:]))}
        self.assertEqual(styles, {False, True})

    def test_thinking_and_running_command_animate_without_new_chunks(self):
        """无参数；实时思考和命令持续变帧，完成后停止；正文与输入位置保持稳定，返回无。"""
        with ActivityModel() as model:
            with TerminalSession(model.config(), columns=100, rows=40,
                                 ready=lambda t: "Enter a message" in t.text()) as terminal:
                gate = terminal.root / "finish-command"
                model.command = ("printf 'PULSE-OUTPUT\\n'; "
                    f"while [ ! -f {shlex.quote(str(gate))} ]; do sleep 0.02; done")
                terminal.send(b"Inspect pulse\r")
                terminal.wait_for(lambda: "THINK-CURRENT" in terminal.text())
                thinking = self.markers(terminal, "Thinking")
                self.assertGreater(len(thinking), 1, thinking)
                self.assertTrue(thinking <= set("◇◈◆"), thinking)
                model.advance.set()
                terminal.wait_for(lambda: "PULSE-OUTPUT" in terminal.text() and "Running" in terminal.text())
                self.tool_pulses(terminal, "Running", "\uf489")
                gate.touch()
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assertEqual(len(self.markers(terminal, "Ran")), 1)

    def test_preparing_command_and_read_animate_without_new_arguments(self):
        """无参数；命令专用分支与普通读取分支在参数暂停时均持续变帧，返回无。"""
        for tool, arguments in [("run_command", {"command": "printf ready"}),
                                ("read_file", {"path": "fixture.rs"})]:
            with self.subTest(tool=tool), ActivityModel(tool, arguments) as model:
                payload = json.dumps(arguments)
                model.argument_chunks = (payload[:-2], payload[-2:])
                model.advance.set()
                with TerminalSession(model.config(), columns=100, rows=40,
                                     ready=lambda t: "Enter a message" in t.text()) as terminal:
                    terminal.send(b"Inspect preparation\r")
                    terminal.wait_for(lambda: "Preparing" in terminal.text())
                    self.tool_pulses(terminal, "Preparing", "\uf489" if tool == "run_command" else "\uf15c")


if __name__ == "__main__":
    unittest.main()
