"""【终端】【工具阅读回归】固定读取标题、保留代码颜色，并验证输入样式命令。"""

import json
import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import click, lines


class ToolReadingTests(unittest.TestCase):
    """【终端】【工具阅读回归】通过真实伪终端验证标题与正文独立绘制。"""

    def test_sticky_read_title_keeps_syntax_colors_and_remains_clickable(self):
        """无参数；主会话滚动固定标题，正文悬停保留着色，点击固定标题可收起；返回无。"""
        with ActivityModel("read_file", {"path": "sample.rs"}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=24,
                    arguments=["--lang", "en-US"]) as terminal:
                (terminal.root / "sample.rs").write_text("\n".join(
                    f'let value_{row} = "READ-CODE-{row:03}";' for row in range(100)))
                terminal.send(b"Inspect local code\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=20)
                terminal.wait_for(lambda: "Shift+Tab" in lines(terminal)[-1])
                # 1. 【终端】【主会话滚动】回到会话开头后翻页，进入读取正文
                terminal.send(b"\x1b[1;5H\x1b[6~")
                terminal.wait_for(lambda: "Read file" in lines(terminal)[1])
                code_rows = [row for row, line in enumerate(lines(terminal))
                    if row > 1 and "READ-CODE" in line]
                self.assertTrue(code_rows, terminal.text())
                styles = {row: [terminal.screen.buffer[row][col]
                    for col in range(96)] for row in code_rows}
                colors = {cell.fg for cells in styles.values() for cell in cells if cell.data.strip()}
                self.assertGreater(len(colors), 1, "读取正文应保留多种语法颜色")
                # 2. 【终端】【正文悬停】只改变固定标题，代码单元格的全部样式保持原值
                title_column = lines(terminal)[1].index("Read file") + 2
                title_color = terminal.screen.buffer[1][title_column].fg
                terminal.send(f"\x1b[<35;12;{code_rows[0] + 1}M".encode())
                terminal.wait_for(lambda: terminal.screen.buffer[1][title_column].fg != title_color)
                for row, cells in styles.items():
                    self.assertEqual([terminal.screen.buffer[row][col] for col in range(96)], cells)
                # 3. 【终端】【标题点击】固定标题仍使用同一收起入口
                click(terminal, title_column, 1)
                terminal.wait_for(lambda: "READ-CODE" not in terminal.text())

    def test_input_style_commands_are_registered_and_persisted(self):
        """无参数；补全展示两种输入样式，执行后保存配置；返回无。"""
        with TerminalSession(columns=100, rows=24,
                arguments=["--view", "fullscreen", "--lang", "en-US"]) as terminal:
            terminal.send(b"/tui ")
            terminal.wait_for(lambda: "/tui input panel" in terminal.text()
                and "/tui input separator" in terminal.text())
            terminal.send(b"\x7f" * 5)
            terminal.wait_for(lambda: "/tui " not in terminal.text())
            config_path = terminal.root / "config/sai/config.jsonc"
            for style in ["separator", "panel"]:
                terminal.send(f"/tui input {style}".encode())
                terminal.wait_for(lambda: f"→ /tui input {style}" in terminal.text())
                terminal.send(b"\r")
                terminal.wait_for(lambda: json.loads(config_path.read_text())["display"]["input_style"] == style)


if __name__ == "__main__":
    unittest.main()
