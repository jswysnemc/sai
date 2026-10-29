"""【终端】【分片模型夹具】控制两段响应的到达时间和 HTTP 字节边界。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading


class FragmentedModel:
    """提供可暂停的本地模型响应，不访问外部接口。"""

    def __init__(self, first, second="\nSTREAM-DONE\n", byte_chunks=False):
        """first/second 为两段正文，byte_chunks 指定逐字节 HTTP 分块；返回夹具实例。"""
        self.ready = threading.Event()
        self.release = threading.Event()
        model = self

        class Handler(BaseHTTPRequestHandler):
            """发送固定 SSE 内容，并等待测试确认首段画面。"""
            protocol_version = "HTTP/1.1"

            def log_message(self, *_args):
                """参数为访问日志；返回无，忽略无关输出。"""

            def do_POST(self):
                """无参数；接收请求并按测试设定发送分片响应。"""
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Transfer-Encoding", "chunked")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    self.emit(first, fragmented=byte_chunks)
                    model.ready.set()
                    model.release.wait(10)
                    self.emit(second, reason="stop")
                    self.chunk(b"data: [DONE]\n\n")
                    self.wfile.write(b"0\r\n\r\n")
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

            def emit(self, text, reason=None, fragmented=False):
                """text 为正文，reason 为结束原因，fragmented 指定逐字节分块；返回无。"""
                data = {"choices": [{"index": 0, "delta": {"content": text}, "finish_reason": reason}]}
                payload = ("data: " + json.dumps(data, ensure_ascii=False) + "\n\n").encode()
                for part in (bytes([byte]) for byte in payload) if fragmented else (payload,):
                    self.chunk(part)

            def chunk(self, data):
                """data 为一块原始 HTTP 正文；返回无，立即提交分块及其长度边界。"""
                self.wfile.write(f"{len(data):x}\r\n".encode() + data + b"\r\n")
                self.wfile.flush()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        """无参数；启动本地接口，返回夹具实例。"""
        self.thread.start()
        return self

    def __exit__(self, *_args):
        """参数为退出异常信息；返回无，释放响应并关闭接口线程。"""
        self.release.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)

    def config(self):
        """无参数；返回使用该本地接口的隔离应用配置。"""
        return {
            "active_provider": "test",
            "providers": [{"id": "test", "display_name": "Fragment fixture",
                           "base_url": f"http://127.0.0.1:{self.server.server_port}/v1",
                           "api_key": "fixture", "models": ["test"], "default_model": "test"}],
            "session": {"auto_title_enabled": False},
            "display": {"math_images": False},
        }
