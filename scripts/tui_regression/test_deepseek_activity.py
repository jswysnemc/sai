"""【终端】【执行链回归】验证线性分组、自动折叠、嵌套展开与实时详情。"""

import shlex
import unittest

from mock_activity_model import ActivityModel
from mock_tool_model import MockToolModel
from terminal import TerminalSession
from test_fullscreen import TurnModel, click, content_lines, lines


class DeepseekActivityTests(unittest.TestCase):
    """使用隔离伪终端检查真实按键和鼠标行为，不访问外部模型。"""

    def row_of(self, terminal, needle):
        """参数 terminal 为伪终端，needle 为标记；返回首个匹配行号。"""
        return next(row for row, text in enumerate(lines(terminal)) if needle in text)

    def test_reasoning_and_running_tool_form_one_linear_chain(self):
        """无参数；近期工具和思考保留详情，手动折叠及整组折叠保留子节点选择。"""
        with ActivityModel() as model, TerminalSession(model.config(), columns=100, rows=40,
                arguments=["--lang", "en-US"]) as terminal:
            gate = terminal.root / "release-command"
            model.command = ("printf 'CURRENT-STDOUT\\n'; "
                f"for attempt in $(seq 1 400); do [ -f {shlex.quote(str(gate))} ] && break; sleep 0.05; done; "
                "printf 'FINAL-STDOUT\\n'")
            terminal.send(b"Inspect current activity\r")
            terminal.wait_for(model.ready.is_set, timeout=15)
            terminal.wait_for(lambda: "THINK-CURRENT" in terminal.text())
            self.assertIn("Thinking", terminal.text())
            self.assertNotIn("Execution details", terminal.text())
            model.advance.set()
            terminal.wait_for(lambda: "1 tool · 1 thought" in terminal.text(), timeout=15)
            terminal.wait_for(lambda: any(line.strip() == "└ CURRENT-STDOUT" for line in content_lines(terminal)))
            self.assertIn("THINK-CURRENT", terminal.text())
            self.assertIn("Thought", terminal.text())
            gate.touch()
            terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
            terminal.pump(.4)
            self.assertTrue(any(line.strip() == "└ CURRENT-STDOUT" for line in content_lines(terminal)))
            self.assertIn("FINAL-STDOUT", terminal.text())
            click(terminal, 5, self.row_of(terminal, "Thought"))
            terminal.wait_for(lambda: "THINK-CURRENT" not in terminal.text())
            click(terminal, 5, self.row_of(terminal, "Thought"))
            terminal.wait_for(lambda: "THINK-CURRENT" in terminal.text())
            click(terminal, 5, self.row_of(terminal, "1 tool · 1 thought"))
            terminal.wait_for(lambda: "Thought" not in terminal.text())
            self.assertNotIn("THINK-CURRENT", terminal.text())
            self.assertIn("ANSWER-END", terminal.text())
            click(terminal, 5, self.row_of(terminal, "1 tool · 1 thought"))
            terminal.wait_for(lambda: "THINK-CURRENT" in terminal.text())
            click(terminal, 5, self.row_of(terminal, "Ran"))
            terminal.wait_for(lambda: "FINAL-STDOUT" not in terminal.text())
            click(terminal, 5, self.row_of(terminal, "Ran"))
            terminal.wait_for(lambda: "FINAL-STDOUT" in terminal.text())
            click(terminal, 8, self.row_of(terminal, "FINAL-STDOUT"))
            terminal.wait_for(lambda: "FINAL-STDOUT" not in terminal.text())

    def test_file_read_summary_matches_reference_layout(self):
        """无参数；文件与思考以折叠链排列，摘要显示路径、行范围和实际耗时。"""
        with ActivityModel("read_file", {"path": "fixture.txt", "offset": 1, "limit": 60}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                (terminal.root / "fixture.txt").write_text("\n".join(f"FILE-CONTENT-{index}" for index in range(60)))
                terminal.send(b"Read the fixture\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                terminal.pump(.4)
                terminal.send(b"\x1b[1;3H")
                terminal.wait_for(lambda: "Thought" in terminal.text())
                group = self.row_of(terminal, "▾ 1 read · 1 thought")
                thought = self.row_of(terminal, "Thought")
                tool = self.row_of(terminal, "\uf15c Read file")
                self.assertLess(group, thought)
                self.assertLess(thought, tool)
                self.assertEqual(thought, group + 1)
                self.assertEqual(tool, thought + 1)
                self.assertIn("fixture.txt (L1–60)", lines(terminal)[tool])
                self.assertIn("ms", lines(terminal)[tool])
                symbol_column = lines(terminal)[tool].index("\uf15c")
                self.assertEqual(terminal.screen.buffer[tool][symbol_column].fg, "green")
                self.assertNotIn("FILE-CONTENT-30", terminal.text())
                self.assertNotIn("THINK-CURRENT", terminal.text())
                self.assertNotIn("Execution details", terminal.text())
                terminal.send(b"\x1b[1;3F")
                terminal.wait_for(lambda: "FILE-CONTENT-59" in terminal.text())
                terminal.send(b"\x1b[5;3~")
                terminal.wait_for(lambda: "FILE-CONTENT-30" in terminal.text())
                self.assertIn("\uf15c Read file", terminal.text())

    def test_nested_wheel_scroll_keeps_group_and_composer_fixed(self):
        """无参数；链内滚动可回看详情，新增输出不打断浏览，滚动到底部恢复跟随。"""
        with MockToolModel("run_command") as model:
            config = model.config()
            config["display"] = {"tui_style": "deepseek"}
            with TerminalSession(config, columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect many details\r")
                terminal.wait_for(model.ready.is_set)
                model.fragments.put('{"command":"\n' + "\n".join(f"DETAIL-{index:02}" for index in range(30)) + "\n")
                terminal.wait_for(lambda: "DETAIL-29" in terminal.text())
                group = self.row_of(terminal, "Preparing")
                composer = self.row_of(terminal, "→")
                wheel_row = group + 5
                terminal.send(f"\x1b[<64;8;{wheel_row + 1}M".encode())
                terminal.wait_for(lambda: "DETAIL-19" in terminal.text() and "DETAIL-29" not in terminal.text())
                self.assertIn("Preparing", terminal.text())
                self.assertEqual(self.row_of(terminal, "Preparing"), group)
                self.assertEqual(self.row_of(terminal, "→"), composer)
                self.assertIn("Click to follow latest", terminal.text())
                model.fragments.put("\n".join(f"DETAIL-{index:02}" for index in range(30, 35)) + "\n")
                terminal.pump(.5)
                self.assertIn("DETAIL-19", terminal.text())
                self.assertNotIn("DETAIL-34", terminal.text())
                for _ in range(3):
                    terminal.send(f"\x1b[<65;8;{wheel_row + 1}M".encode())
                    terminal.pump(.1)
                terminal.wait_for(lambda: "DETAIL-34" in terminal.text())
                self.assertIn("Following latest", terminal.text())
                self.assertEqual(self.row_of(terminal, "Preparing"), group)
                self.assertEqual(self.row_of(terminal, "→"), composer)

    def test_live_arguments_remain_in_chain_on_resize(self):
        """无参数；长参数在链内显示最新片段，缩放保留当前节点与输入光标。"""
        with MockToolModel("write_file") as model:
            config = model.config()
            config["display"] = {"tui_style": "deepseek"}
            with TerminalSession(config, columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Write the fixture\r")
                terminal.wait_for(model.ready.is_set)
                model.fragments.put('{"path":"fixture.txt","content":"FIRST-DETAIL\\n')
                terminal.wait_for(lambda: "FIRST-DETAIL" in terminal.text())
                self.assertNotIn("1 tool", terminal.text())
                model.fragments.put("old-" * 4000 + "\\nLATEST-LONG-DETAIL\\n")
                terminal.wait_for(lambda: "LATEST-LONG-DETAIL" in terminal.visible_text().replace("│", "").replace("┃", ""))
                terminal.resize(50, 18)
                terminal.wait_for(lambda: "LATEST-LONG-DETAIL" in terminal.visible_text().replace("│", "").replace("┃", ""))
                self.assertNotIn("Execution details", terminal.text())
                self.assertLess(terminal.screen.cursor.y, 18)

    def test_single_historical_reasoning_stays_expanded(self):
        """无参数；唯一思考使用五行窗口，滚轮可查看其余正文，点击正文不会折叠。"""
        with TurnModel() as model:
            config = model.config()
            config["display"]["tui_style"] = "deepseek"
            with TerminalSession(config, columns=100, rows=40,
                    arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"ALPHA\r")
                terminal.wait_for(lambda: "ANSWER-ALPHA-7" in terminal.text(), timeout=15)
                terminal.pump(.4)
                terminal.send(b"\x1b[1;5H")
                terminal.wait_for(lambda: "Thought" in terminal.text())
                terminal.wait_for(lambda: "THINK-ALPHA-0" in terminal.text())
                title = self.row_of(terminal, "Thought")
                composer = self.row_of(terminal, "→")
                self.assertEqual(sum("THINK-ALPHA-" in line for line in lines(terminal)), 5)
                terminal.send(f"\x1b[<65;8;{title + 3}M".encode())
                terminal.wait_for(lambda: "THINK-ALPHA-6" in terminal.text())
                self.assertEqual(self.row_of(terminal, "Thought"), title)
                self.assertEqual(self.row_of(terminal, "→"), composer)
                self.assertNotIn("1 thought", terminal.text())
                click(terminal, 8, self.row_of(terminal, "THINK-ALPHA-6"))
                terminal.pump(.3)
                self.assertIn("THINK-ALPHA-6", terminal.text())

    def test_slash_command_returns_to_linear_view(self):
        """无参数；独占命令结束后恢复全屏，底部不残留旧详情面板。"""
        with TerminalSession(columns=100, rows=30, arguments=["--lang", "en-US", "--style", "deepseek"]) as terminal:
            terminal.send(b"/help\r")
            terminal.wait_for(lambda: "/style" in terminal.text() and "Esc close" in terminal.text())
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "Esc close" not in terminal.text() and "Ctrl+O" in lines(terminal)[0])
            self.assertNotIn("Execution details", terminal.text())
            self.assertIn("→", terminal.text())


if __name__ == "__main__":
    unittest.main()
