"""【终端】【图片回归】验证真实读图工具的折叠、Kitty 预览和双层滚动。"""

import re
import struct
import unittest
import zlib

from mock_activity_model import ActivityModel
from kitty_assertions import placements
from terminal import TerminalSession
from test_fullscreen import click, lines


def png_image(path):
    """【终端】【图片样本】参数 path 为目标路径；创建无外部依赖的 PNG，返回无。"""
    def chunk(kind, data):
        """参数 kind 为块类型、data 为数据；返回带校验和的 PNG 块。"""
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    width, height = 640, 480
    pixels = (b"\x00" + b"\x2f\x79\xc5" * width) * height
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
                    + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))


class DeepseekImageTests(unittest.TestCase):
    """【终端】【图片回归】使用隔离配置和本地模型，不访问外部服务。"""

    def test_read_image_expansion_scroll_resize_and_collapse(self):
        """无参数；读取图片默认展开，滚动和调整尺寸保留预览，手动收起清理放置。"""
        with ActivityModel("read_file", {}) as model:
            with TerminalSession(model.config(), columns=90, rows=20,
                    arguments=["--lang", "zh-CN"], kitty_graphics=True) as terminal:
                path = terminal.root / "sample.png"
                png_image(path)
                model.arguments = {"path": str(path)}
                terminal.send(b"Inspect image\r")
                terminal.wait_for(model.ready.is_set)
                model.advance.set()
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assertIn("Viewed image", terminal.text())
                terminal.wait_for(lambda: b"\x1b_Ga=p" in terminal.raw)
                self.assertIn(b"\x1b_Ga=t", terminal.raw)
                image_row, image_column, _image_height = placements(bytes(terminal.raw), 90, 20)[-1]
                self.assertEqual(image_column, 4)
                self.assertLess(image_row, next(index for index, line in enumerate(lines(terminal)) if "→" in line))
                self.assertNotIn("iVBOR", terminal.text())
                self.assertIn("\u2192", terminal.text())
                for key in [b"\x1b[1;3H", b"\x1b[1;3F"]:
                    before = len(terminal.raw)
                    terminal.send(key)
                    terminal.pump(.4)
                    # 1. 【终端图片】【放置复用】无位移时复用原放置，删除旧图后必须重新放置
                    if b"\x1b_Ga=d" in terminal.raw[before:]:
                        self.assertIn(b"\x1b_Ga=p", terminal.raw[before:])
                    self.assertIn("Viewed image", terminal.text())
                terminal.resize(65, 30)
                terminal.pump(.5)
                self.assertIn("Viewed image", terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "Viewed image" in line)
                before = len(terminal.raw)
                click(terminal, 8, row)
                terminal.pump(.5)
                self.assertIn(b"\x1b_Ga=d", terminal.raw[before:])
                self.assertNotIn(b"\x1b_Ga=p", terminal.raw[before:])
                self.assertIn("ANSWER-END", terminal.text())

    def test_list_markers_and_digit_boundary_alignment(self):
        """无参数；真实文本读取保持行号与代码对齐，模型无序列表区分层级标记。"""
        with ActivityModel("read_file", {}) as model:
            model.answer = "ANSWER-END\n- first item\n* second item\n  + nested item"
            with TerminalSession(model.config(), columns=100, rows=32,
                    arguments=["--lang", "en-US"]) as terminal:
                path = terminal.root / "fixture.rs"
                path.write_text("\n".join(f"let value_{index} = {index};" for index in range(1, 13)))
                model.arguments = {"path": str(path), "offset": 8, "limit": 4}
                terminal.send(b"Inspect text\r")
                terminal.wait_for(model.ready.is_set)
                model.advance.set()
                terminal.wait_for(lambda: "second item" in terminal.text())
                self.assertIn("- first item", terminal.text())
                self.assertIn("- second item", terminal.text())
                self.assertIn("· nested item", terminal.text())
                terminal.wait_for(lambda: "value_10" in terminal.text())
                code = [line for line in lines(terminal) if re.search(r"\b(?:8|9|10|11)\s+let value_", line)]
                self.assertEqual(len(code), 4)
                self.assertEqual(len({line.index("let") for line in code}), 1)
                self.assertEqual(len({re.search(r"\d", line).start() for line in code}), 1)

    def test_protocol_filter_handles_split_transmission(self):
        """无参数；跨数据块的 Kitty APC 不进入 pyte 正文，但原始输出仍由会话保存。"""
        terminal = TerminalSession.__new__(TerminalSession)
        terminal.graphics_pending = ""
        self.assertEqual(terminal.filter_graphics("before\x1b"), "before")
        self.assertEqual(terminal.filter_graphics("_Ga=t;DATA"), "")
        self.assertEqual(terminal.filter_graphics("\x1b\\after"), "after")


if __name__ == "__main__":
    unittest.main()
