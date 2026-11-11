"""【CodeMode】【端到端回归】使用隔离配置和本地模型替身验证实际 Sai 主程序。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest


class CodeModeAgentTests(unittest.TestCase):
    """覆盖模型声明、并行读取、输出过滤、状态提交和失败恢复。"""

    def execute(self, scripts, timeout_ms=60000, mode="only", deferred=False):
        """【CodeMode】【会话运行】scripts 为脚本或原生调用，mode 为策略，deferred 指定延迟读取；返回结果与请求。"""
        requests = []

        class Handler(BaseHTTPRequestHandler):
            """以 OpenAI 兼容事件流返回固定脚本，不访问外部服务。"""

            def log_message(self, *_args):
                """【CodeMode】【请求日志】参数为 HTTP 日志字段；不输出常规测试请求。"""

            def do_POST(self):
                """【CodeMode】【模型回复】读取当前请求并返回下一个脚本或最终正文。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                index = sum("tools" in item for item in requests)
                requests.append(request)
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                if "tools" in request and index < len(scripts):
                    item = scripts[index]
                    name, arguments = ("codemode", {"code": item}) if isinstance(item, str) else (item["name"], item["arguments"])
                    delta = {"tool_calls": [{"index": 0, "id": f"script-{index}",
                             "type": "function", "function": {"name": name,
                             "arguments": json.dumps(arguments)}}]}
                    reason = "tool_calls"
                else:
                    delta = {"content": "integration-complete"}
                    reason = "stop"
                for change, finish in [(delta, None), ({}, reason)]:
                    event = {"choices": [{"index": 0, "delta": change, "finish_reason": finish}]}
                    self.wfile.write(("data: " + json.dumps(event) + "\n\n").encode())
                self.wfile.write(b"data: [DONE]\n\n")
                self.wfile.flush()

        with tempfile.TemporaryDirectory(prefix="sai-codemode-test-") as temporary:
            root = Path(temporary)
            (root / "one.txt").write_text("RAW-FILLER-NEVER-IN-CONTEXT\n" * 100 + "needle")
            (root / "two.txt").write_text("needle")
            server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                config = {"provider_setup_complete": True, "active_provider": "fixture",
                          "providers": [{"id": "fixture", "display_name": "Fixture",
                          "base_url": f"http://127.0.0.1:{server.server_port}/v1",
                          "api_key": "local-fixture", "models": ["fixture"], "default_model": "fixture"}],
                          "codemode": {"timeout_ms": timeout_ms},
                          "skills": {"enabled": False}, "load_instruction_files": False,
                          "display": {"fullscreen": False}}
                if mode is not None:
                    config["codemode"]["mode"] = mode
                if deferred:
                    config.update(agents=[{"id": "fixture-agent", "name": "Fixture", "enabled_tools": ["read_file"], "deferred_tools": ["read_file"]}],
                                  cli_agent="fixture-agent", default_agent="fixture-agent")
                target = root / "config/sai/config.jsonc"
                target.parent.mkdir(parents=True)
                target.write_text(json.dumps(config))
                environment = dict(os.environ)
                for key, folder in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                                    ("XDG_STATE_HOME", "state"), ("XDG_CACHE_HOME", "cache")]:
                    environment[key] = str(root / folder)
                environment["TERM"] = "dumb"
                binary = Path(os.environ.get("SAI_TEST_BINARY", Path(__file__).resolve().parents[1] / "target/debug/sai"))
                result = subprocess.run([str(binary), "--lang", "en-US", "ask", "Execute the regression scripts"],
                                        cwd=root, env=environment, text=True, capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("integration-complete", result.stdout)
                self.assertGreaterEqual(len(requests), len(scripts) + 1)
                for request in requests:
                    if "tools" not in request:
                        continue
                    names = [tool["function"]["name"] for tool in request.get("tools", [])]
                    if mode == "off":
                        self.assertNotIn("codemode", names)
                    else:
                        self.assertIn("codemode", names)
                    if mode == "only":
                        self.assertTrue(set(names) <= {"codemode", "load", "request_capability"}, names)
                    elif not deferred:
                        self.assertIn("read_file", names)
                return result, [request for request in requests if "tools" in request]
            finally:
                server.shutdown()
                server.server_close()
                thread.join()

    @unittest.skipUnless(os.name == "posix", "XDG configuration isolation requires a POSIX host")
    def test_parallel_tools_filter_results_and_commit_branch_state(self):
        """【CodeMode】【成功链路】无参数；验证真实工具、精简上下文与下次脚本的状态读取。"""
        _, requests = self.execute([
            "const xs = await Promise.all([tools.read_file({path:'one.txt'}), tools.read_file({path:'two.txt'})]); "
            "store('count', xs.length); return 'filtered:' + xs.every(x => JSON.stringify(x).includes('needle'));",
            "return load('count');",
        ])
        outputs = [message["content"] for message in requests[-1]["messages"] if message["role"] == "tool"]
        self.assertTrue(any("filtered:true" in value for value in outputs), outputs)
        self.assertTrue(any("Script completed\n2" in value for value in outputs), outputs)
        self.assertNotIn("RAW-FILLER-NEVER-IN-CONTEXT", json.dumps(requests[-1]["messages"]))

    @unittest.skipUnless(os.name == "posix", "XDG configuration isolation requires a POSIX host")
    def test_timeout_keeps_partial_output_and_host_accepts_next_script(self):
        """【CodeMode】【失败恢复】无参数；验证死循环截止、部分输出及失败状态不提交。"""
        _, requests = self.execute([
            "store('failed', true); text('partial-output'); while(true) {}",
            "return load('failed') === undefined ? 'store-clean' : 'store-leaked';",
        ], timeout_ms=1500)
        outputs = [message["content"] for message in requests[-1]["messages"] if message["role"] == "tool"]
        self.assertTrue(any("partial-output" in value and "timed out" in value for value in outputs), outputs)
        self.assertTrue(any("store-clean" in value for value in outputs), outputs)

    @unittest.skipUnless(os.name == "posix", "XDG configuration isolation requires a POSIX host")
    def test_default_hybrid_uses_native_and_script_calls_with_one_description(self):
        """【CodeMode】【混合链路】无参数；验证缺省配置下两种真实调用成功，声明没有复制。"""
        _, requests = self.execute([
            {"name": "read_file", "arguments": {"path": "two.txt"}},
            "return 'script:' + String(await tools.read_file({path:'two.txt'})).includes('needle');",
        ], mode=None)
        for request in requests:
            definitions = {item["function"]["name"]: item["function"] for item in request["tools"]}
            self.assertNotIn(definitions["read_file"]["description"], definitions["codemode"]["description"])
        outputs = [message["content"] for message in requests[-1]["messages"] if message["role"] == "tool"]
        self.assertTrue(any("needle" in value for value in outputs), outputs)
        self.assertTrue(any("script:true" in value for value in outputs), outputs)

    @unittest.skipUnless(os.name == "posix", "XDG configuration isolation requires a POSIX host")
    def test_deferred_schema_is_shared_by_invoke_and_script(self):
        """【CodeMode】【加载链路】无参数；验证隐藏工具不能从脚本发现，加载一次后两条执行路径均可用。"""
        for mode in ["on", "only"]:
            calls = [
                "return {hidden: !ALL_TOOLS.some(t=>t.name==='read_file'), absent: (await describeTool('read_file')) === undefined};",
                {"name": "load", "arguments": {"type": "tool", "keywords": ["read_file"]}},
                "return 'script:' + String(await tools.read_file({path:'two.txt'})).includes('needle');",
            ]
            if mode == "on":
                calls.append({"name": "invoke_tool", "arguments": {"tool_name": "read_file", "arguments": {"path": "two.txt"}}})
            with self.subTest(mode=mode):
                _, requests = self.execute(calls, mode=mode, deferred=True)
                outputs = [message["content"] for message in requests[-1]["messages"] if message["role"] == "tool"]
                self.assertTrue(any('"hidden":true' in value and '"absent":true' in value for value in outputs), outputs)
                self.assertTrue(any("script:true" in value for value in outputs), outputs)
                for request in requests:
                    self.assertNotIn("read_file", [item["function"]["name"] for item in request["tools"]])
                    loaded = [message for message in request["messages"] if message["role"] == "tool" and '"definition"' in str(message["content"])]
                    self.assertLessEqual(len(loaded), 1)


if __name__ == "__main__":
    unittest.main()
