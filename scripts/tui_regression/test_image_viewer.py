"""【图片浮窗】【终端回归】通过真实读图、鼠标点击和窗口缩放验证放大交互。"""

import re
import unittest

from kitty_assertions import placements
from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_activity_images import png_image
from test_fullscreen import click, lines


class ImageViewerTests(unittest.TestCase):
    """【图片浮窗】【回归环境】仅使用本地模型与隔离终端，不访问外部服务。"""

    def test_image_click_enlarges_resize_and_close_preserve_draft(self):
        """无参数；点击缩略图放大，缩放后关闭按钮仍有效，关闭恢复正文和输入草稿。"""
        with ActivityModel("read_file", {}) as model:
            with TerminalSession(model.config(), columns=90, rows=32,
                    arguments=["--lang", "en-US"], kitty_graphics=True) as terminal:
                path = terminal.root / "sample.png"
                png_image(path)
                model.arguments = {"path": str(path)}
                terminal.send(b"Inspect image\r")
                terminal.wait_for(model.ready.is_set)
                model.advance.set()
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                terminal.wait_for(lambda: b"\x1b_Ga=p" in terminal.raw)
                image_row, image_column, _ = placements(bytes(terminal.raw), 90, 32)[-1]
                terminal.send(b"kept draft")
                terminal.wait_for(lambda: "kept draft" in terminal.text())
                before = len(terminal.raw)
                click(terminal, image_column + 1, image_row + 1)
                terminal.wait_for(lambda: "Image · sample.png" in terminal.text())
                enlarged = bytes(terminal.raw[before:])
                widths = [int(width) for width in re.findall(rb"a=p[^\x1b]*,c=(\d+)", enlarged)]
                self.assertTrue(widths and max(widths) > 24, widths)
                self.assertIn(b"\x1b_Ga=d", enlarged)
                self.assertNotIn(b"sai-image;", terminal.raw)
                terminal.send(b"do not insert\r")
                terminal.pump(.3)
                self.assertIn("Image · sample.png", terminal.text())
                self.assertNotIn("do not insert", terminal.text())
                terminal.resize(65, 25)
                terminal.pump(.6)
                title_row = next(index for index, line in enumerate(lines(terminal))
                                 if "Image · sample.png" in line)
                close_column = lines(terminal)[title_row].index("×")
                click(terminal, close_column, title_row)
                terminal.wait_for(lambda: "Viewed image" in terminal.text())
                self.assertNotIn("Image · sample.png", terminal.text())
                self.assertIn("kept draft", terminal.text())
                terminal.wait_for(lambda: b"\x1b_Ga=p" in terminal.raw[before:])

    def test_character_image_click_and_escape(self):
        """无参数；无图片协议时字符缩略图也能打开大图，Esc 关闭且不退出会话。"""
        with ActivityModel("read_file", {}) as model:
            with TerminalSession(model.config(), columns=80, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                path = terminal.root / "sample.png"
                png_image(path)
                model.arguments = {"path": str(path)}
                terminal.send(b"Inspect image\r")
                terminal.wait_for(model.ready.is_set)
                model.advance.set()
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                title_row = next(index for index, line in enumerate(lines(terminal))
                                 if "Viewed image" in line)
                click(terminal, 5, title_row + 2)
                terminal.wait_for(lambda: "Image · sample.png" in terminal.text())
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "Viewed image" in terminal.text())
                self.assertNotIn("Image · sample.png", terminal.text())
                terminal.send(b"editable")
                terminal.wait_for(lambda: "editable" in terminal.text())


if __name__ == "__main__":
    unittest.main()
