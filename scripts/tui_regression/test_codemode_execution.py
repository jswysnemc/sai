"""【CodeMode】【终端回归】验证独立权限、嵌套审批、截止处理和 JavaScript 着色。"""

import json
import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession


class CodeModeExecutionTests(unittest.TestCase):
    """使用本地模型替身和临时工作目录驱动真实主程序。"""

    def configuration(self, model, timeout_ms=10000, fullscreen=True):
        """参数为模型替身、截止毫秒和界面布局；返回隔离配置。"""
        config = model.config()
        config.update(codemode={"timeout_ms": timeout_ms}, load_instruction_files=False,
                      sandbox={"enabled": False})
        config["display"]["fullscreen"] = fullscreen
        model.reasoning = ""
        model.advance.set()
        return config

    def audit(self, terminal):
        """参数 terminal 为测试终端；返回该临时会话的权限审计记录。"""
        return [json.loads(line) for path in terminal.root.rglob("permission-audit.jsonl")
                for line in path.read_text().splitlines()]

    def terminal(self, config, permission="--audited"):
        """参数为配置和权限标志；返回同时开启脚本方式的测试终端。"""
        return TerminalSession(config, columns=120, rows=36,
                               arguments=["--lang", "en-US", permission, "--code-model"],
                               ready=lambda terminal: "CODE-MODEL" in terminal.text())

    def test_nested_permission_is_visible_and_allow_executes_only_after_decision(self):
        """无参数；验证两种布局均展示真实命令，获批前不执行，获批后完成。"""
        code = 'return await tools.run_command({command:"printf approved > result.txt"});'
        for fullscreen in [True, False]:
            with self.subTest(fullscreen=fullscreen), ActivityModel("codemode", {"code": code}) as model:
                with self.terminal(self.configuration(model, fullscreen=fullscreen)) as terminal:
                    terminal.send(b"Execute the script\r")
                    terminal.wait_for(lambda: "Allow once" in terminal.text())
                    self.assertIn("printf approved", terminal.text())
                    self.assertIn("run_command", terminal.text())
                    self.assertFalse((terminal.root / "result.txt").exists())
                    terminal.send(b"1")
                    terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                    self.assertEqual((terminal.root / "result.txt").read_text(), "approved")

    def test_deny_never_executes_nested_command(self):
        """无参数；验证拒绝嵌套命令后脚本返回且不会创建目标文件。"""
        code = 'return await tools.run_command({command:"printf denied > result.txt"});'
        with ActivityModel("codemode", {"code": code}) as model:
            with self.terminal(self.configuration(model)) as terminal:
                terminal.send(b"Execute the script\r")
                terminal.wait_for(lambda: "Allow once" in terminal.text())
                terminal.send(b"2")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assertFalse((terminal.root / "result.txt").exists())
                self.assertTrue(any(record["decision"] == "denied" for record in self.audit(terminal)))

    def test_script_deadline_expires_while_permission_is_pending(self):
        """无参数；验证无键盘输入时截止仍生效，审批消失且命令没有执行。"""
        code = 'return await tools.run_command({command:"printf expired > result.txt"});'
        with ActivityModel("codemode", {"code": code}) as model:
            with self.terminal(self.configuration(model, timeout_ms=1200)) as terminal:
                terminal.send(b"Execute the script\r")
                terminal.wait_for(lambda: "Allow once" in terminal.text())
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=6)
                self.assertFalse((terminal.root / "result.txt").exists())
                self.assertNotIn("Allow once", terminal.text())
                self.assertIn("timed out", terminal.text())

    def test_yolo_and_codemode_execute_together_without_permission_prompt(self):
        """无参数；验证显式 YOLO 与脚本方式共存，写入成功且无需审批。"""
        code = 'return await tools.run_command({command:"printf yolo > result.txt"});'
        with ActivityModel("codemode", {"code": code}) as model:
            with self.terminal(self.configuration(model), "--yolo") as terminal:
                self.assertIn("YOLO + CODE-MODEL", terminal.text())
                terminal.send(b"Execute the script\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assertEqual((terminal.root / "result.txt").read_text(), "yolo")
                self.assertFalse(any(record["decision"] == "requested" for record in self.audit(terminal)))

    def test_shift_tab_during_approval_preserves_codemode(self):
        """无参数；验证审批期间切到 YOLO 后保留脚本方式并继续执行。"""
        code = 'return await tools.run_command({command:"printf switched > result.txt"});'
        with ActivityModel("codemode", {"code": code}) as model:
            with self.terminal(self.configuration(model)) as terminal:
                terminal.send(b"Execute the script\r")
                terminal.wait_for(lambda: "Allow once" in terminal.text())
                terminal.send(b"\x1b[Z")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assertIn("yolo+code-model", terminal.text().lower())
                self.assertEqual((terminal.root / "result.txt").read_text(), "switched")

    def test_plan_and_codemode_keep_readonly_restrictions(self):
        """无参数；验证计划模式下脚本无法通过嵌套命令写入文件。"""
        code = 'return await tools.run_command({command:"printf blocked > result.txt"});'
        with ActivityModel("codemode", {"code": code}) as model:
            with self.terminal(self.configuration(model), "--plan") as terminal:
                terminal.send(b"Execute the script\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assertFalse((terminal.root / "result.txt").exists())
                self.assertNotIn("Allow once", terminal.text())
                self.assertIn("blocked", terminal.text().lower())

    def test_running_and_completed_code_have_syntax_colors(self):
        """无参数；验证运行中与完成后的代码正文包含多种词法颜色。"""
        code = 'const result = await tools.run_command({command:"sleep 1; printf ready"});\ntext(result);'
        with ActivityModel("codemode", {"code": code}) as model:
            with self.terminal(self.configuration(model), "--yolo") as terminal:
                terminal.send(b"Execute the script\r")
                terminal.wait_for(lambda: "const result" in terminal.text())
                self.assert_code_colors(terminal)
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assert_code_colors(terminal)

    def assert_code_colors(self, terminal):
        """参数 terminal 为已绘制代码的终端；验证正文字符颜色至少两种，返回无。"""
        row = next(index for index, line in enumerate(terminal.screen.display) if "const result" in line)
        colors = {character.fg for character in terminal.screen.buffer[row].values() if character.data.isalnum()}
        self.assertGreater(len(colors), 1, terminal.text())
