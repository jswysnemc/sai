"""【会话性能】【操作测量】测量真实创建和切换请求，不修改用户数据。"""

import json
import statistics
import time
import urllib.request


def request_json(origin, path, method="GET", payload=None):
    """调用隔离服务；参数为地址、路径、方法及请求体，返回 JSON 响应。"""
    body = None if payload is None else json.dumps(payload).encode()
    request = urllib.request.Request(
        origin + path, data=body, method=method,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def measure_session_actions(origin, count, repeats):
    """测量创建、切换；参数为隔离地址、基准会话数和重复次数，返回统计列表。"""
    samples = {"create": [], "switch": []}
    for index in range(repeats):
        # 1. 【会话性能】【创建】计时包含服务端持久化及响应接收
        started = time.perf_counter()
        created = request_json(origin, "/api/sessions", "POST", {"title": f"Action sample {index}"})
        samples["create"].append((time.perf_counter() - started) * 1000)
        session_id = created["id"]
        assert session_id != "default" and created["active"], created
        try:
            # 2. 【会话性能】【切换】每次从新会话切回固定样本，避免重复选择当前会话
            started = time.perf_counter()
            switched = request_json(origin, "/api/sessions/default/switch", "POST", {})
            samples["switch"].append((time.perf_counter() - started) * 1000)
            assert switched["id"] == "default" and switched["active"], switched
            sessions = request_json(origin, "/api/sessions")
            assert len(sessions) == count + 1
            assert [entry["id"] for entry in sessions if entry["active"]] == ["default"]
        finally:
            # 3. 【会话性能】【清理样本】恢复规模，删除耗时不计入创建和切换
            request_json(origin, f"/api/sessions/{session_id}", "DELETE")
    return [
        {
            "kind": action, "sessions": count,
            "median_ms": round(statistics.median(values), 3),
            "samples_ms": [round(value, 3) for value in values],
        }
        for action, values in samples.items()
    ]
