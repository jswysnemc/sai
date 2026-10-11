"""【终端队列】【模型替身】按测试信号结束首轮，再让首条排队消息失败或等待中断。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
import threading


class QueueModel:
    """只记录合成请求，提供可重复的队列失败边界。"""

    def __init__(self, interrupt=False):
        """interrupt 决定排队首条等待中断还是返回请求错误；返回无。"""
        self.started = threading.Event()
        self.release = threading.Event()
        self.queue_started = threading.Event()
        self.queue_release = threading.Event()
        self.requests = []
        self.received = []
        model = self

        class Handler(BaseHTTPRequestHandler):
            """为真实终端提供本地流式模型接口。"""

            def log_message(self, *_args):
                """忽略访问日志；参数为日志字段，返回无。"""

            def do_POST(self):
                """按最后一个测试标记响应请求；无参数，返回无。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                text = "\n".join(str(message.get("content")) for message in request["messages"]
                                 if message.get("role") == "user")
                tags = re.findall(r"\b(INITIAL|QUEUED_FAILURE|QUEUED_LONG|QUEUED_LAST)\b", text)
                tag = tags[-1] if tags else "UNKNOWN"
                if not request.get("stream"):
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Connection", "close")
                    self.end_headers()
                    self.wfile.write(json.dumps({"choices": [{"message": {"role": "assistant",
                        "content": "fixture"}, "finish_reason": "stop"}]}).encode())
                    return
                # 1. 【终端队列】【请求分类】会话记忆也使用流式接口，不能把辅助请求计为重复提交
                if text.startswith("Update the session memory for future turns."):
                    tag = "MEMORY"
                else:
                    model.requests.append(request)
                    model.received.append(tag)
                if tag == "QUEUED_FAILURE" and not interrupt:
                    model.queue_started.set()
                    self.send_response(400)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Connection", "close")
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": {"message": "QUEUE-FAILURE-FIXTURE",
                                                         "type": "invalid_request_error"}}).encode())
                    return
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    if tag == "INITIAL":
                        self.emit({"content": "INITIAL-START\n"})
                        model.started.set()
                        if not model.release.wait(20):
                            return
                    elif tag == "QUEUED_FAILURE":
                        self.emit({"content": "QUEUED-START\n"})
                        model.queue_started.set()
                        if not model.queue_release.wait(20):
                            return
                    self.emit({"content": f"{tag}-DONE\n"})
                    self.emit({}, "stop")
                    self.wfile.write(b"data: [DONE]\n\n")
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

            def emit(self, delta, finish=None):
                """发送 delta 增量和可选结束原因 finish；返回无。"""
                chunk = {"choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(chunk) + "\n\n").encode())
                self.wfile.flush()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """启动本地服务器；无参数，返回模型替身。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_args):
        """释放所有等待并关闭服务器；参数为异常信息，返回无。"""
        self.release.set()
        self.queue_release.set()
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """返回禁用外部服务和自动标题的隔离配置；无参数。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture",
            "display_name": "Fixture", "base_url": f"http://127.0.0.1:{self.server.server_port}/v1",
            "api_key": "local-fixture", "models": ["fixture"], "default_model": "fixture"}],
            "session": {"auto_title_enabled": False}, "skills": {"enabled": False},
            "tools": {"enabled": False}, "memory": {"enabled": False},
            "permission": {"tui_mode": "yolo", "cli_mode": "yolo"}}
