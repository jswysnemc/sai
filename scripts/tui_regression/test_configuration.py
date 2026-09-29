"""【TUI配置】【交互回归】验证慢模型接口期间的列表导航与长文本可见性。"""

import http.server
import threading
import unittest

from terminal import TerminalSession


class ConfigurationTests(unittest.TestCase):
    """使用真实伪终端验证配置和输入界面。"""

    def test_provider_columns_respond_before_models_finish(self):
        """模型接口挂起时仍能立即向右切栏；无参数，返回无。"""
        release = threading.Event()
        entered = threading.Event()

        class Handler(http.server.BaseHTTPRequestHandler):
            """延迟响应的本地模型目录。"""

            def do_GET(self):
                """等待测试释放再返回空模型列表；返回无。"""
                entered.set()
                release.wait(12)
                try:
                    self.send_response(200)
                    self.end_headers()
                    self.wfile.write(b'{"data": []}')
                except OSError:
                    pass

            def log_message(self, *_args):
                """忽略无关访问日志；参数为日志内容，返回无。"""

        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        config = {"active_provider": "test", "providers": [{
            "id": "test", "display_name": "Slow fixture", "api_key": "fixture",
            "base_url": f"http://127.0.0.1:{server.server_port}/v1",
            "models": ["local-model"], "default_model": "local-model"}, {
            "id": "second", "display_name": "Second fixture", "api_key": "fixture",
            "base_url": f"http://127.0.0.1:{server.server_port}/v1",
            "models": ["second-local"], "default_model": "second-local"}]}
        try:
            with TerminalSession(config=config, columns=50, rows=24) as terminal:
                terminal.send(b"/config\r")
                terminal.wait_for(lambda: "SAI CONFIG" in terminal.text())
                terminal.send(b"2\r")
                terminal.wait_for(lambda: ("PROVIDERS & MODELS" in terminal.text() or "供应商与模型" in terminal.text()))
                terminal.wait_for(entered.is_set)
                terminal.send(b"\x1b[C")
                terminal.wait_for(lambda: ("ORG" in terminal.text() or "组织" in terminal.text()), timeout=.8)
                terminal.send(b"\x1b[C")
                terminal.wait_for(lambda: "local-model" in terminal.text(), timeout=.8)
                terminal.send(b"\x1b[D\x1b[D\x1b[B\x1b[C\x1b[C")
                terminal.wait_for(lambda: "second-local" in terminal.text(), timeout=.8)
                terminal.send(b"\x1b[D\x1b[D\x1b[A\x1b[C\x1b[C")
                terminal.wait_for(lambda: "local-model" in terminal.text(), timeout=.8)
                terminal.send(b"\x1b[D\x1b[D\r")
                terminal.wait_for(lambda: "编辑供应商" in terminal.text() or "EDIT PROVIDER" in terminal.text())
                terminal.send(b"4\r")
                terminal.wait_for(lambda: "模型连接测试" in terminal.text() or "MODEL CONNECTION TEST" in terminal.text())
                terminal.send(b"q")
                terminal.wait_for(lambda: "编辑供应商" in terminal.text() or "EDIT PROVIDER" in terminal.text(), timeout=.8)
        finally:
            release.set()
            server.shutdown()
            server.server_close()

    def test_long_input_keeps_the_cursor_line_visible(self):
        """长文本移动到开头后显示光标所在内容，而非固定折叠首段；返回无。"""
        with TerminalSession(columns=50, rows=24) as terminal:
            terminal.pump(.5)
            for chunk in [b"BEGIN-EDIT "] + [b"long-input " * 10] * 10 + [b" END-EDIT"]:
                terminal.send(chunk)
                terminal.pump(.08)
            terminal.pump(.6)
            terminal.send(b"\x01")
            terminal.wait_for(lambda: "BEGIN-EDIT" in terminal.screen.display[terminal.screen.cursor.y], timeout=1)
            terminal.send(b"Z")
            terminal.wait_for(lambda: "ZBEGIN-EDIT" in terminal.text(), timeout=1)
            terminal.send(b"\x05")
            terminal.wait_for(lambda: "END-EDIT" in terminal.screen.display[terminal.screen.cursor.y], timeout=1)

    def test_context_alias_edits_and_cancels_without_saving(self):
        """别名进入上下文面板，直接替换数值并取消；返回无。"""
        with TerminalSession() as terminal:
            terminal.send(b"/content edit\r")
            terminal.wait_for(lambda: "占用比例" in terminal.text() or "Usage ratio" in terminal.text())
            terminal.send(b"\r85")
            terminal.wait_for(lambda: "85|" in terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: "85%" in terminal.text())
            terminal.resize(42, 12)
            terminal.pump(.3)
            terminal.send(b"q")
            terminal.wait_for(lambda: "已取消压缩" in terminal.text() or "settings cancelled" in terminal.text())
            terminal.send(b"AFTER-CANCEL")
            terminal.wait_for(lambda: "AFTER-CANCEL" in terminal.screen.display[terminal.screen.cursor.y])
