"""【终端】【工具模型替身】按测试指令分段发送文件工具参数。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import queue
import threading


class MockToolModel:
    """提供逐段控制的工具参数流，隔离真实模型和用户文件。"""

    def __init__(self, tool_name):
        """tool_name 为工具名称；初始化参数队列和本地服务器。"""
        self.fragments = queue.Queue()
        self.ready = threading.Event()
        model = self

        class Handler(BaseHTTPRequestHandler):
            """返回由测试控制的 OpenAI 兼容工具调用流。"""

            def log_message(self, *_):
                """忽略常规请求日志；参数为标准日志字段，返回无。"""

            def do_POST(self):
                """按队列逐段输出参数，收到结束标记后关闭连接；返回无。"""
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    self.emit({"index": 0, "id": "fixture-tool", "type": "function",
                               "function": {"name": tool_name, "arguments": ""}})
                    model.ready.set()
                    while True:
                        fragment = model.fragments.get(timeout=20)
                        if fragment is None:
                            return
                        self.emit({"index": 0, "function": {"arguments": fragment}})
                except (BrokenPipeError, ConnectionResetError, queue.Empty):
                    pass

            def emit(self, call):
                """call 为工具调用增量；写入一个 SSE 事件，返回无。"""
                data = {"id": "fixture", "object": "chat.completion.chunk", "choices": [
                    {"index": 0, "delta": {"tool_calls": [call]}, "finish_reason": None}]}
                self.wfile.write(("data: " + json.dumps(data) + "\n\n").encode())
                self.wfile.flush()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """启动本地服务器，返回模型替身。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_):
        """结束待处理请求并关闭服务器；参数为异常信息，返回无。"""
        self.fragments.put(None)
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """返回仅连接本地服务器的配置，工具参数保持未完成状态。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture",
            "display_name": "Fixture", "base_url": f"http://127.0.0.1:{self.server.server_port}/v1",
            "api_key": "local-fixture", "models": ["fixture"], "default_model": "fixture"}],
            "tools": {"enabled": True}, "skills": {"enabled": False},
            "permission": {"tui_mode": "yolo"}}
