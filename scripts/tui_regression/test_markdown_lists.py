"""【终端】【列表排版】验证流式与定稿正文中的列表层级、颜色和悬挂缩进。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import lines


class MarkdownListTests(unittest.TestCase):
    """【终端】【列表回归】宽窄终端均使用真实 Markdown 渲染路径。"""

    def test_nested_lists_keep_hanging_indent_and_distinct_markers(self):
        """无参数；嵌套列表区分标记，换行对齐正文，复杂组之间保留空行，返回无。"""
        for style in ["classic", "deepseek"]:
            for columns in [48, 100]:
                with self.subTest(style=style, columns=columns), ActivityModel("read_file", {"path": "fixture.txt"}) as model:
                    model.advance.set()
                    model.answer = ("- **`check_os_info`**\n  - CHILD " + "确认流程状态正确。" * 5
                                    + "\n    CONTINUATION\n    - LEAF detail\n- NEXT group")
                    config = model.config()
                    config["display"].update(tui_style=style, fullscreen=True)
                    with TerminalSession(config, columns=columns, rows=40,
                                         arguments=["--lang", "en-US"],
                                         ready=lambda t: "Ctrl+O" in t.text()) as terminal:
                        (terminal.root / "fixture.txt").write_text("fixture")
                        terminal.pump(.4)
                        terminal.send(b"List layout\r")
                        terminal.wait_for(lambda: "NEXT group" in terminal.text())
                        rendered = lines(terminal)
                        parent = next(line for line in rendered if "check_os_info" in line)
                        child = next(i for i, line in enumerate(rendered) if "CHILD" in line)
                        continuation = next(i for i, line in enumerate(rendered) if "CONTINUATION" in line)
                        leaf = next(line for line in rendered if "LEAF" in line)
                        next_group = next(i for i, line in enumerate(rendered) if "NEXT group" in line)
                        self.assertTrue(parent.lstrip().startswith("- check_os_info"), parent)
                        self.assertTrue(rendered[child].lstrip().startswith("· CHILD"), rendered[child])
                        self.assertTrue(leaf.lstrip().startswith("– LEAF"), leaf)
                        column = rendered[child].index("CHILD")
                        for line in rendered[child + 1:continuation + 1]:
                            self.assertEqual(len(line) - len(line.lstrip()), column, line)
                        self.assertFalse(rendered[next_group - 1].strip(" │┃╶─"), rendered[next_group - 1])
                        row = next(i for i, line in enumerate(rendered) if "check_os_info" in line)
                        code_column = rendered[row].index("check_os_info")
                        self.assertNotEqual(terminal.screen.buffer[row][code_column].fg, "d7afff")


if __name__ == "__main__":
    unittest.main()
