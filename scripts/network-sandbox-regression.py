"""验证实际沙箱命令的网络授权和直接连接限制。"""
import base64
import json
import os
import pathlib
import subprocess
import sys
import tempfile


def main():
    """读取二进制路径并运行隔离探针；无参数，断言失败时退出非零状态。"""
    binary = pathlib.Path(os.environ.get("SAI_TEST_BINARY", "target/debug/sai")).resolve()
    with tempfile.TemporaryDirectory() as root:
        root = str(pathlib.Path(root).resolve())
        policy = dict(
            access="WorkspaceWrite", cwd=root, writable_roots=[root],
            write_protected=[], deny_read=[], network=True,
            allowed_domains=["example.com"], temp_dir=None, scrubbed_env=[],
        )
        probe = pathlib.Path(root) / "probe.py"
        probe.write_text('''import socket, sys, urllib.request, urllib.error
if sys.platform == 'linux':
    try:
        socket.socket(socket.AF_UNIX)
    except PermissionError:
        pass
    else:
        raise AssertionError('Unix socket bypass')
try:
    connection = socket.create_connection(("1.1.1.1", 443), 2)
except OSError:
    pass
else:
    connection.close()
    raise AssertionError("direct network bypass")
try:
    urllib.request.urlopen("https://unapproved.invalid", timeout=5)
except urllib.error.URLError as error:
    assert "403" in str(error), str(error)
else:
    raise AssertionError("domain bypass")
print("Direct network and unapproved domains blocked")
''')
        launch = dict(policy=policy, shell="/bin/sh", script=f"'{sys.executable}' '{probe}'")
        payload = base64.b64encode(json.dumps(launch).encode()).decode()
        subprocess.run([str(binary), "--internal-network-sandbox", payload], check=True, timeout=30)


if __name__ == "__main__":
    main()
