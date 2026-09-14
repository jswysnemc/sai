"""【会话载入】【性能复现】用隔离目录测量正式服务的列表和历史接口。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request


def fixture_paths(root):
    """构造隔离路径；参数 root 为临时根目录，返回工作区、会话目录和子进程环境。"""
    workspace = root / "workspace"
    workspace.mkdir()
    workspace_id = hashlib.sha256(str(workspace.resolve()).encode()).hexdigest()[:16]
    state = root / "state" / "sai" / "sessions" / "workspaces" / workspace_id
    state.mkdir(parents=True)
    process_env = dict(os.environ)
    process_env.update(
        XDG_CONFIG_HOME=str(root / "config"),
        XDG_DATA_HOME=str(root / "data"),
        XDG_STATE_HOME=str(root / "state"),
        XDG_CACHE_HOME=str(root / "cache"),
        XDG_PICTURES_DIR=str(root / "pictures"),
        SAI_LANG="en_US",
    )
    return workspace, state, process_env


def seed_sessions(state, count):
    """写入合成会话索引；参数 state 为隔离目录、count 为数量，返回无。"""
    records = [
        {
            "id": "default" if index == 0 else f"bench_{index:06}",
            "title": f"Synthetic session {index}",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
        }
        for index in range(count)
    ]
    # 1. 【会话载入】【隔离样本】原子替换，避免服务读到部分写入的索引
    pending = state / "index.pending"
    pending.write_text(json.dumps(records), encoding="utf-8")
    pending.replace(state / "index.json")
    (state / "current").write_text("default\n", encoding="utf-8")


def seed_turns(state, count, content_bytes):
    """写入完整的线性历史；参数依次为隔离目录、轮数和每轮正文长度，返回无。"""
    database = state / "data" / "default" / "conversation.db"
    with sqlite3.connect(database) as connection:
        connection.execute("DELETE FROM turns")
        connection.executemany(
            """INSERT INTO turns (
                turn_id, seq, user_content, user_timestamp, assistant_content,
                assistant_timestamp, status, parent_turn_id
            ) VALUES (?, ?, ?, ?, ?, ?, 'completed', ?)""",
            (
                (
                    f"turn_{index}",
                    index,
                    f"Synthetic request {index}",
                    "2026-01-01T00:00:00Z",
                    "Synthetic response\n" + "x" * content_bytes,
                    "2026-01-01T00:00:01Z",
                    "__root__" if index == 1 else f"turn_{index - 1}",
                )
                for index in range(1, count + 1)
            ),
        )
        connection.execute(
            "INSERT OR REPLACE INTO session_tree_meta (key, value) VALUES ('active_leaf', ?)",
            (f"turn_{count}" if count else "__root__",),
        )


def read_json(url):
    """请求本机测试接口；参数 url 为隔离服务地址，返回 JSON 和响应字节数。"""
    with urllib.request.urlopen(url, timeout=60) as response:
        body = response.read()
    return json.loads(body), len(body)


def measure(url, repeats):
    """测量请求延迟；参数为地址和重复次数，返回耗时、字节数及最后一次响应。"""
    samples = []
    payload = None
    response_bytes = 0
    for _ in range(repeats):
        started = time.perf_counter()
        payload, response_bytes = read_json(url)
        samples.append((time.perf_counter() - started) * 1000)
    return {
        "median_ms": round(statistics.median(samples), 3),
        "samples_ms": [round(value, 3) for value in samples],
        "response_bytes": response_bytes,
    }, payload


def wait_ready(process, origin, log_path):
    """等待隔离服务启动；参数为进程、地址和日志路径，成功返回无，失败抛错。"""
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and process.poll() is None:
        try:
            read_json(origin + "/api/sessions")
            return
        except (OSError, urllib.error.URLError):
            time.sleep(0.1)
    raise RuntimeError("Isolated server did not start:\n" + log_path.read_text()[-2000:])


def run_benchmark(args, root):
    """运行正式接口测量；参数为命令行配置和临时根目录，返回测量记录列表。"""
    workspace, state, process_env = fixture_paths(root)
    seed_sessions(state, 1)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    origin = f"http://127.0.0.1:{port}"
    log_path = root / "server.log"
    records = []
    with log_path.open("w") as log:
        process = subprocess.Popen(
            [
                str(args.binary.resolve()), "web", "--port", str(port),
                "--host", "127.0.0.1", "--allow-anonymous", "--no-open",
                "--workspace", str(workspace),
            ],
            cwd=workspace, env=process_env, stdout=log, stderr=subprocess.STDOUT,
        )
        try:
            wait_ready(process, origin, log_path)
            # 1. 【会话载入】【列表测量】只改变会话数量，保留相同服务和请求形式
            for count in args.sessions:
                seed_sessions(state, count)
                for endpoint in ["/api/sessions", "/api/sessions/tree"]:
                    result, payload = measure(origin + endpoint, args.repeats)
                    if endpoint.endswith("sessions"):
                        actual = len(payload)
                    else:
                        matching = [
                            group for group in payload
                            if Path(group["workspace_path"]).resolve() == workspace.resolve()
                        ]
                        assert len(matching) == 1
                        actual = len(matching[0]["sessions"])
                    assert actual == count, (actual, count)
                    result.update(kind="list", endpoint=endpoint, sessions=count)
                    records.append(result)
                    print(json.dumps(result), flush=True)
            # 2. 【会话载入】【历史测量】固定会话数与返回轮数，只增加已有历史规模
            seed_sessions(state, 1)
            read_json(origin + "/api/sessions/default/timeline?limit=1")
            for count in args.turns:
                seed_turns(state, count, args.content_bytes)
                for endpoint in ["timeline", "messages"]:
                    result, payload = measure(
                        f"{origin}/api/sessions/default/{endpoint}?limit={args.limit}",
                        args.repeats,
                    )
                    if endpoint == "timeline":
                        assert len(payload["turns"]) == min(args.limit, count)
                        assert payload["turns"][-1]["turn_id"] == f"turn_{count}"
                    result.update(kind="history", endpoint=endpoint, turns=count)
                    records.append(result)
                    print(json.dumps(result), flush=True)
                if args.tree:
                    result, payload = measure(
                        f"{origin}/api/sessions/default/turn-tree", args.repeats
                    )
                    assert len(payload["nodes"]) == count
                    assert payload["total_turns"] == count
                    result.update(kind="tree", turns=count)
                    records.append(result)
                    print(json.dumps(result), flush=True)
                if args.tui:
                    from session_load_tui import measure_tui_history
                    result = measure_tui_history(args.binary, workspace, state, process_env)
                    result.update(kind="tui", turns=count)
                    records.append(result)
                    print(json.dumps(result), flush=True)
        except Exception:
            if process.poll() is not None:
                print(log_path.read_text()[-1800:], flush=True)
            raise
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
    return records


def main():
    """解析测量参数并校验预算；无参数，返回无，超出预算时退出码为 1。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/sai"))
    parser.add_argument("--sessions", type=int, nargs="*", default=[100, 500, 1000])
    parser.add_argument("--turns", type=int, nargs="*", default=[100, 1000, 5000])
    parser.add_argument("--content-bytes", type=int, default=8192)
    parser.add_argument("--limit", type=int, default=40)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--max-list-ms", type=float)
    parser.add_argument("--max-history-ms", type=float)
    parser.add_argument("--tui", action="store_true", help="Also measure a Unix PTY; requires pyte")
    parser.add_argument("--tree", action="store_true", help="Also verify the complete branch index")
    parser.add_argument("--max-terminal-bytes", type=int)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("This isolated benchmark requires Linux/XDG directories")
    if args.repeats < 1 or any(count < 1 for count in args.sessions + args.turns):
        parser.error("repeats and fixture counts must be positive")
    with tempfile.TemporaryDirectory(prefix="sai-session-load-") as directory:
        records = run_benchmark(args, Path(directory))
    if args.output:
        args.output.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    failed = [
        record for record in records
        if (budget := {"list": args.max_list_ms, "history": args.max_history_ms}.get(record["kind"]))
        is not None and record["median_ms"] > budget
    ]
    failed.extend(record for record in records if record["kind"] == "tui"
                  and args.max_terminal_bytes is not None
                  and record["output_bytes"] > args.max_terminal_bytes)
    if failed:
        print(f"Latency budget exceeded for {len(failed)} measurements", flush=True)
        raise SystemExit(1)


if __name__ == "__main__":
    main()
