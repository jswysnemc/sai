"""用普通外部插件和本地 HTTP 服务验收任意来源读取及正文转换。"""

import argparse
import hashlib
import json
import shutil
import sys
import tempfile
import time
from pathlib import Path

from fixture import Fixture
from http_server import Server


def read(fixture, url, *, succeeds=True):
    """【网络验收】【外部调用】接收隔离环境、地址及预期状态，返回预览对象或错误。"""
    result = fixture.plugins("run", "url-preview", "read", url, succeeds=succeeds)
    return json.loads(result["output"]) if succeeds else result


def verify(fixture, example, server):
    """【网络验收】【公共契约】接收普通包和本地服务，验证授权、响应与转换，不返回数据。"""
    source = fixture.root / "url-preview"
    shutil.copytree(example, source)
    manifest_path = source / "sai-plugin.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["capabilities"]["http"] = [server.origin]
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    fixture.plugins("check", source)
    fixture.plugins("install", source)
    fixture.plugins("enable", "url-preview")
    read(fixture, server.origin + "/page", succeeds=False)
    assert server.requests == []
    fixture.plugins("enable", "url-preview", "--allow-http-read-any")
    page = read(fixture, server.origin + "/page")
    assert page["status"] == 200
    assert "公共接口" in page["document"]["text"]
    assert "<h1>" not in page["document"]["text"]
    redirected = read(fixture, server.origin + "/redirect")
    assert redirected["url"] == server.origin + "/page"
    long = read(fixture, server.origin + "/long")["document"]
    assert long == {"text": "中" * 2000, "total_chars": 3000, "truncated": True}
    error = read(fixture, server.origin + "/error")
    assert error["status"] == 503 and error["document"] is None
    assert "exceeds byte limit" in read(fixture, server.origin + "/large", succeeds=False)
    before_loop = len(server.requests)
    assert "redirect limit" in read(fixture, server.origin + "/loop", succeeds=False)
    assert len(server.requests) - before_loop == 4
    started = time.monotonic()
    read(fixture, server.origin + "/slow", succeeds=False)
    assert time.monotonic() - started < 8

    # 1. 【网络验收】【独立撤权】关闭任意来源权限后只保留显式的精确来源
    fixture.plugins("enable", "url-preview", "--allow-http", server.origin)
    fixture.plugins("enable", "url-preview", "--no-http-read-any")
    info = fixture.plugins("info", "url-preview")["plugins"][0]
    assert info["source"]["kind"] == "installed"
    assert not info["effective_grants"].get("http_read_any")
    assert info["effective_grants"]["http"] == [server.origin]
    assert read(fixture, server.origin + "/page")["status"] == 200
    with Server() as other:
        read(fixture, other.origin + "/page", succeeds=False)
        assert other.requests == []
    fixture.plugins("enable", "url-preview", "--no-http")
    count = len(server.requests)
    read(fixture, server.origin + "/page", succeeds=False)
    assert len(server.requests) == count
    fixture.plugins("remove", "url-preview")


def main():
    """【网络验收】【执行入口】读取程序和报告路径，保存结果、耗时及请求证据，无返回值。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    arguments = parser.parse_args()
    if sys.platform != "linux":
        parser.error("This fixture requires Linux XDG isolation.")
    binary = arguments.binary.resolve(strict=True)
    report = arguments.report.resolve()
    example = Path(__file__).resolve().parents[2] / "examples/lua-plugins/url-preview"
    evidence = {"binary": str(binary), "passed": False}
    with binary.open("rb") as stream:
        evidence["binary_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="sai-http-smoke-") as directory, Server() as server:
        fixture = Fixture(binary, Path(directory))
        evidence.update({"working_directory": str(fixture.project), "calls": fixture.calls,
                         "requests": server.requests})
        try:
            verify(fixture, example, server)
            evidence["passed"] = True
        finally:
            evidence["seconds"] = round(time.monotonic() - started, 3)
            report.parent.mkdir(parents=True, exist_ok=True)
            report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Passed {len(fixture.calls)} CLI calls and {len(server.requests)} HTTP requests in {evidence['seconds']}s")


if __name__ == "__main__":
    main()
