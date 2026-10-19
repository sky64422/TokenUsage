"""Install the opt-in global AGY hook, preserving unrelated configuration."""
import datetime
import json
from pathlib import Path
import shutil
import subprocess
import sys

NAME = "tokenusage-models"


def install(home, executable):
    config = home / ".gemini/config/hooks.json"
    destination = home / ".gemini/config/tokenusage/agy-token-hook.py"
    data = json.loads(config.read_text(encoding="utf-8-sig")) if config.exists() else {}
    if not isinstance(data, dict):
        raise ValueError("AGY hooks configuration must be an object")
    command = subprocess.list2cmdline([str(executable), str(destination)])
    definition = {"PostInvocation": [{"type": "command", "command": command, "timeout": 5}]}
    if NAME in data and data[NAME] != definition:
        raise ValueError("Existing tokenusage hook differs; inspect it before replacement")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(Path(__file__).with_name("agy-token-hook.py"), destination)
    if config.exists() and data.get(NAME) != definition:
        stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
        shutil.copyfile(config, config.with_name("hooks.json." + stamp + ".bak"))
    data[NAME] = definition
    temporary = config.with_suffix(".tokenusage.tmp")
    temporary.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    temporary.replace(config)
    return config


if __name__ == "__main__":
    print("Installed AGY model metadata hook:", install(Path.home(), Path(sys.executable)))
