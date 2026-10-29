"""【终端】【UTF-8 边界回归】损坏 ESC 后的多字节正文不能引发主线程崩溃。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading
import unittest

from terminal import TerminalSession


class AnsiUtf8Tests(unittest.TestCase):
    """通过真实模型流式入口覆盖内联和全屏渲染。"""

    def test_malformed_escape_keeps_multibyte_text_and_input_usable(self):
        """无参数；接口返回 ESC 加替换字符，正文完整显示，回复结束后仍能输入。"""
        source = "0123456789abcdef\x1b\ufffdtail"
        visible = "0123456789abcdef\ufffdtail"

        class Handler(BaseHTTPRequestHandler):
            """返回固定的损坏转义文本，不访问外部接口。"""

            def log_message(self, *_args):
                """参数为访问日志；返回无，忽略无关输出。"""

            def do_POST(self):
                """无参数；接收请求并发送带非法转义类型的流式正文。"""
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                data = {"choices": [{"index": 0, "delta": {"content": source}, "finish_reason": "stop"}]}
                try:
                    self.wfile.write(("data: " + json.dumps(data) + "\n\ndata: [DONE]\n\n").encode())
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        config = {
            "active_provider": "test",
            "providers": [{
                "id": "test", "display_name": "UTF-8 fixture",
                "base_url": f"http://127.0.0.1:{server.server_port}/v1",
                "api_key": "fixture", "models": ["test"], "default_model": "test",
            }],
            "session": {"auto_title_enabled": False},
            "display": {"math_images": False},
            "tools": {"enabled": False},
            "skills": {"enabled": False},
        }
        try:
            for view in ("inline", "fullscreen"):
                with self.subTest(view=view), TerminalSession(
                    config, arguments=["--lang", "en-US", "--view", view], columns=100, rows=32,
                ) as terminal:
                    terminal.send(b"Inspect UTF-8 text\r")
                    terminal.wait_for(lambda: visible in terminal.text(), timeout=5)
                    terminal.send(b"AFTER-UTF8")
                    terminal.wait_for(lambda: "AFTER-UTF8" in terminal.text(), timeout=5)
                    self.assertNotIn("panicked at", terminal.text(history=True))
        finally:
            server.shutdown()
            server.server_close()
