"""【桌面测试】【进程夹具】只处理合成 MCP 请求，不访问真实桌面或配置。"""

import json
import os
from pathlib import Path
import sys
import time


def main():
    """从标准输入读取 MCP 请求并写入夹具响应；无参数，无返回值。"""
    records = Path(os.environ["SAI_FIXTURE_RECORDS"])
    state = json.loads(os.environ["SAI_FIXTURE_STATE"])
    for line in sys.stdin:
        request = json.loads(line)
        with records.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps({"pid": os.getpid(), "request": request}) + "\n")
        if "id" not in request:
            continue
        if request["method"] == "initialize":
            result = {"protocolVersion": "2025-11-25", "capabilities": {"tools": {}},
                      "serverInfo": {"name": "native-fixture", "version": "1"}}
        elif request["params"]["name"] == "get_window_state":
            result = state
        else:
            time.sleep(30)
            result = {"content": [{"type": "text", "text": "fixture input finished"}]}
        print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)


if __name__ == "__main__":
    main()
