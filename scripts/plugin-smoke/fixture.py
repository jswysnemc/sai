"""为外部 Lua 插件的命令行验收提供隔离目录和执行记录。"""

import json
import os
import subprocess
import time
from pathlib import Path


class Fixture:
    """封装一次验收使用的程序、独立配置、工作目录和命令证据。"""

    def __init__(self, binary: Path, root: Path):
        """【插件验收】【隔离环境】接收程序及临时根目录，初始化独立 XDG 路径，无返回值。"""
        self.binary = binary
        self.root = root
        self.project = root / "project"
        self.source = root / "project-notes"
        self.environment = os.environ.copy()
        for variable, directory in {
            "XDG_CONFIG_HOME": "config",
            "XDG_DATA_HOME": "data",
            "XDG_CACHE_HOME": "cache",
            "XDG_STATE_HOME": "state",
            "XDG_PICTURES_DIR": "pictures",
            "TMPDIR": "tmp",
        }.items():
            path = root / directory
            path.mkdir()
            self.environment[variable] = str(path)
        self.project.mkdir()
        self.config = root / "config/sai/plugins.jsonc"
        self.calls = []

    def plugins(self, *arguments, mode="plan", succeeds=True):
        """【插件验收】【真实命令】接收插件参数、权限模式及预期状态，返回 JSON 或失败文本。"""
        command = [str(self.binary), "--lang", "en-US", f"--{mode}",
                   "plugins", "--json", *map(str, arguments)]
        started = time.monotonic()
        result = subprocess.run(
            command, cwd=self.project, env=self.environment,
            capture_output=True, text=True, timeout=60,
        )
        self.calls.append({
            "command": command,
            "exit_code": result.returncode,
            "seconds": round(time.monotonic() - started, 3),
            "stdout": result.stdout,
            "stderr": result.stderr,
        })
        if (result.returncode == 0) != succeeds:
            raise AssertionError(f"Unexpected command status: {command}\n{result.stdout}\n{result.stderr}")
        return json.loads(result.stdout) if succeeds else result.stdout + result.stderr

    def command(self, name, *, mode="plan", succeeds=True):
        """【插件验收】【用户命令】接收示例命令和权限模式，返回解码结果或错误文本。"""
        result = self.plugins("run", "project-notes", name, mode=mode, succeeds=succeeds)
        return json.loads(result["output"]) if succeeds else result

    def info(self):
        """【插件验收】【有效授权】无参数，返回示例的当前描述和有效授权。"""
        return self.plugins("info", "project-notes")["plugins"][0]

    def configure(self, settings, *, succeeds=True):
        """【插件验收】【设置替换】接收设置对象和预期状态，通过正式配置入口返回保存结果。"""
        path = self.root / "settings.json"
        path.write_text(json.dumps(settings), encoding="utf-8")
        return self.plugins("configure", "project-notes", path, succeeds=succeeds)
