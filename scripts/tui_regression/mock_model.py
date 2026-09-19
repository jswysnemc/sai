"""【终端】【模型替身】通过本地 HTTP 提供可暂停的流式表格。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading
import time


class MockModel:
    """只使用合成内容，供完整终端链路验证。"""

    def __init__(self, row_count=60):
        """row_count 为表格行数；创建本地服务器及暂停事件。"""
        self.ready = threading.Event()
        self.release = threading.Event()
        model = self

        class Handler(BaseHTTPRequestHandler):
            """将模型请求转换为固定的测试增量。"""

            def log_message(self, *_):
                """忽略常规 HTTP 日志；参数为标准日志字段，返回无。"""

            def do_POST(self):
                """返回思考、表格与结束正文；无参数，无返回值。"""
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    self.emit({"reasoning_content": "THOUGHT-PREFIX\nPreserve earlier output.\n"})
                    self.emit({"content": "INTRO-PREFIX\n\n| ROWKEY | VALUE | STATUS |\n|---|---|---|\n"})
                    for index in range(row_count):
                        value = "normal" if index < 12 else "longer description with additional details"
                        self.emit({"content": f"| ROW{index:03} | {value} | ready |\n"})
                        time.sleep(.04)
                    model.ready.set()
                    if not model.release.wait(20):
                        return
                    self.emit({"content": "\nAFTER-TABLE\n"})
                    self.emit({}, "stop")
                    self.wfile.write(b"data: [DONE]\n\n")
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

            def emit(self, delta, finish=None):
                """发送 delta 增量及可选 finish 原因；返回无。"""
                data = {"id": "fixture", "object": "chat.completion.chunk", "choices": [
                    {"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(data) + "\n\n").encode())
                self.wfile.flush()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """启动本地模型服务器，返回实例。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_):
        """释放暂停请求并关闭服务器；参数为异常信息，返回无。"""
        self.release.set()
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """返回仅连接本地服务器的合成配置。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture",
            "display_name": "Fixture", "base_url": f"http://127.0.0.1:{self.server.server_port}/v1",
            "api_key": "local-fixture", "models": ["fixture"], "default_model": "fixture"}],
            "display": {"reasoning": "full"}, "tools": {"enabled": False}, "skills": {"enabled": False}}
