"""为公共 HTTP 契约验收提供有界的本地响应和请求记录。"""

import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    """只处理本地验收所需的固定 GET 路由。"""

    def do_GET(self):
        """【网络验收】【响应样本】记录当前请求并发送固定响应，无参数及返回值。"""
        self.server.requests.append(self.path)
        routes = {
            "/page": (200, "text/html", "<h1>公共接口</h1><p>URL preview</p>"),
            "/long": (200, "text/plain", "中" * 3000),
        }
        if self.path in routes:
            status, content_type, text = routes[self.path]
            body = text.encode()
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path in ("/redirect", "/loop"):
            self.send_response(302)
            self.send_header("Location", "/page" if self.path == "/redirect" else "/loop")
            self.send_header("Content-Length", "0")
            self.end_headers()
        elif self.path in ("/error", "/large", "/slow"):
            self.send_response(503 if self.path == "/error" else 200)
            self.send_header("Content-Length", "5" if self.path == "/slow" else "20000000")
            self.end_headers()
            self.wfile.flush()
            if self.path == "/slow":
                self.server.release.wait(10)
        else:
            self.send_error(404)

    def log_message(self, format, *arguments):
        """【网络验收】【输出控制】接收服务器日志格式和参数，省略默认输出，无返回值。"""


class Server:
    """管理回环监听器、后台服务线程及验收请求记录。"""

    def __enter__(self):
        """【网络验收】【本地启动】无参数，返回使用随机回环端口的服务。"""
        self.http = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.http.requests = []
        self.http.release = threading.Event()
        self.origin = f"http://127.0.0.1:{self.http.server_port}"
        self.thread = threading.Thread(target=self.http.serve_forever, daemon=True)
        self.thread.start()
        return self

    def __exit__(self, *exception):
        """【网络验收】【本地回收】接收上下文退出信息，释放延迟响应和监听器，无返回值。"""
        self.http.release.set()
        self.http.shutdown()
        self.http.server_close()
        self.thread.join(timeout=2)

    @property
    def requests(self):
        """【网络验收】【请求证据】无参数，返回当前已接受请求的路径列表。"""
        return self.http.requests
