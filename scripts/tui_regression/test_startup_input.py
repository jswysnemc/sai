"""【终端】【首次输入】欢迎信息出现时立即提交，启动初始化不能吞掉 Enter。"""
import unittest
from mock_activity_model import ActivityModel
from terminal import TerminalSession


class StartupInputTests(unittest.TestCase):
    """【终端】【守卫交接】同时覆盖全屏与内联启动。"""

    def test_input_is_ready_before_first_welcome_frame(self):
        """无参数；返回无值，按键模式先于欢迎信息，首条输入无需再次按 Enter。"""
        for fullscreen in (False, True):
            with self.subTest(fullscreen=fullscreen), ActivityModel("read_file", {"path":"fixture.txt"}) as model:
                model.advance.set()
                config = model.config()
                config["display"]["fullscreen"] = fullscreen
                with TerminalSession(config, columns=100, rows=32, arguments=["--lang", "en-US"], ready=lambda terminal: "fixture" in terminal.text()) as terminal:
                    (terminal.root / "fixture.txt").write_text("STARTUP-READY")
                    self.assertIn(b"\x1b[?2004h", terminal.raw)
                    self.assertLess(terminal.raw.index(b"\x1b[?2004h"), terminal.raw.index(b"fixture"))
                    terminal.send(b"Check startup input\r")
                    terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
