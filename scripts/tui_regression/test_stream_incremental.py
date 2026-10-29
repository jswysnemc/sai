"""【终端】【流式增量回归】正文不必等待换行即可显示。"""
import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from terminal import TerminalSession


class StreamIncrementalTests(unittest.TestCase):
    """本地流片段验证全屏预览与后续正文连续性。"""

    def test_partial_markdown_appears_without_newline(self):
        """无参数；未换行片段按到达顺序显示，换行后不重复，完成后可继续输入。"""
        sent = threading.Event()
        append = threading.Event()
        release = threading.Event()

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                """参数为访问日志；返回无，避免干扰测试输出。"""
                pass

            def do_POST(self):
                """无参数；分阶段发送未换行正文，等待测试确认屏幕后继续。"""
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    self.emit("正在检查")
                    sent.set()
                    append.wait(10)
                    self.emit("当前仓库")
                    release.wait(10)
                    self.emit("\n继续分析\n", "stop")
                    self.wfile.write(b"data: [DONE]\n\n")
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

            def emit(self, text, reason=None):
                """text 为增量文本，reason 为结束原因；返回无。"""
                data = {"choices": [{"index": 0, "delta": {"content": text}, "finish_reason": reason}]}
                self.wfile.write(("data: " + json.dumps(data) + "\n\n").encode())
                self.wfile.flush()

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        config = {"active_provider": "test", "providers": [{"id": "test", "display_name": "Stream fixture", "base_url": f"http://127.0.0.1:{server.server_port}/v1", "api_key": "fixture", "models": ["test"], "default_model": "test"}], "display": {"fullscreen": False, "math_images": False}, "session": {"auto_title_enabled": False}, "tools": {"enabled": False}, "skills": {"enabled": False}}
        try:
            with TerminalSession(config, arguments=["--lang", "en-US"], columns=100, rows=32) as terminal:
                terminal.send(b"\x0f")
                terminal.wait_for(lambda: b"\x1b[?1049h" in terminal.raw)
                terminal.send(b"Inspect stream\r")
                terminal.wait_for(sent.is_set)
                terminal.wait_for(lambda: "正在检查" in terminal.text())
                self.assertNotIn("当前仓库", terminal.text())
                before = len(terminal.raw)
                append.set()
                terminal.wait_for(lambda: "正在检查当前仓库" in terminal.text())
                self.assertNotIn("正在检查".encode(), terminal.raw[before:])
                self.assertNotIn("继续分析", terminal.text())
                terminal.resize(40, 20)
                terminal.wait_for(lambda: "正在检查当前仓库" in terminal.visible_text())
                release.set()
                terminal.wait_for(lambda: "继续分析" in terminal.visible_text())
                self.assertEqual(terminal.visible_text().count("正在检查当前仓库"), 1)
                terminal.send(b"AFTER-STREAM")
                terminal.wait_for(lambda: "AFTER-STREAM" in terminal.visible_text())
        finally:
            append.set()
            release.set()
            server.shutdown()
            server.server_close()
