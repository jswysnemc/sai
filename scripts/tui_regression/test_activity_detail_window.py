"""【终端】【详情窗口回归】验证工具与思考只有离开十行窗口后才自动折叠。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import lines


class ActivityDetailWindowTests(unittest.TestCase):
    """【终端】【详情窗口回归】通过短工具与长工具检查真实视口中的展开边界。"""

    def test_details_remain_open_until_new_content_fills_the_window(self):
        """无参数；短读取保留思考详情，长读取将旧思考折叠，工具完成仍展示详情。"""
        for count in [1, 12]:
            with self.subTest(count=count), ActivityModel("read_file", {"path": "fixture.txt"}) as model:
                with TerminalSession(model.config(), columns=100, rows=40,
                        arguments=["--lang", "en-US"]) as terminal:
                    (terminal.root / "fixture.txt").write_text("\n".join(f"DETAIL-{index:02}" for index in range(count)))
                    terminal.send(b"Inspect detail window\r")
                    terminal.wait_for(model.ready.is_set)
                    terminal.wait_for(lambda: "THINK-CURRENT" in terminal.text())
                    model.advance.set()
                    terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                    terminal.wait_for(lambda: f"DETAIL-{count - 1:02}" in terminal.text())
                    if count == 1:
                        self.assertIn("THINK-CURRENT", terminal.text())
                        self.assertIn("Detailed current reasoning.", terminal.text())
                    else:
                        header = next(index for index, line in enumerate(lines(terminal)) if "1 read · \uf0eb 1 thought" in line)
                        last = next(index for index, line in enumerate(lines(terminal)) if f"DETAIL-{count - 1:02}" in line)
                        self.assertEqual(last - header, count + 2)
                        self.assertNotIn("Following latest", terminal.text())
                        terminal.send(b"\x1b[1;3H")
                        terminal.wait_for(lambda: "Thought" in terminal.text())
                        self.assertNotIn("THINK-CURRENT", terminal.text())
                        self.assertIn("Read file", terminal.text())


if __name__ == "__main__":
    unittest.main()
