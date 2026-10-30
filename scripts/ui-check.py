"""Run deterministic browser regressions on a private Vite server."""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
PORT = 1420  # Several existing fixtures have this URL embedded.
START_TIMEOUT_SECONDS = 20
SCRIPTS = [
    ("settings", ["settings-smoke.py"]),
    ("detail layout", ["detail-layout-smoke.py"]),
    ("quota states", ["grok-usage-smoke.py"]),
    ("rings", ["notch-ring-smoke.py"]),
    ("usage tracks", ["usage-track-smoke.py"]),
    ("liquid edge coverage", ["liquid-edge-smoke.py"]),
    ("provider token details", ["provider-token-details-smoke.py", "--port", str(PORT)]),
    ("display updates", ["display-update-smoke.py", "--port", str(PORT), "--token"]),
    ("theme geometry", ["theme-transition-smoke.py", "--port", str(PORT), "--token"]),
]


def main():
    output = ROOT / "tmp" / "ui-check"
    output.mkdir(parents=True, exist_ok=True)
    with socket.socket() as sock:
        if sock.connect_ex(("127.0.0.1", PORT)) == 0:
            raise RuntimeError(f"Port {PORT} is in use; stop the other server to avoid testing another checkout")

    vite = ROOT / "node_modules" / "vite" / "bin" / "vite.js"
    server = subprocess.Popen(["node", str(vite), "--host", "127.0.0.1", "--port", str(PORT), "--strictPort"], cwd=ROOT)
    results = []
    try:
        deadline = time.monotonic() + START_TIMEOUT_SECONDS
        while True:
            if server.poll() is not None:
                raise RuntimeError(f"Vite exited with {server.returncode}")
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{PORT}/", timeout=1) as response:
                    if response.status == 200:
                        break
            except OSError:
                if time.monotonic() >= deadline:
                    raise RuntimeError("Vite did not become ready")
                time.sleep(.1)
        env = os.environ.copy()
        env["TOKENUSAGE_TEST_URL"] = f"http://127.0.0.1:{PORT}"
        for name, args in SCRIPTS:
            print(f"\n[UI] {name}", flush=True)
            proc = subprocess.run([sys.executable, "-X", "utf8", str(ROOT / "scripts" / args[0]), *args[1:]],
                                  cwd=ROOT, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
            log = output / (args[0].removesuffix(".py") + ".log")
            log.write_text(proc.stdout + proc.stderr, encoding="utf-8")
            print(proc.stdout + proc.stderr, end="", flush=True)
            results.append({"name": name, "passed": proc.returncode == 0, "exit_code": proc.returncode, "log": str(log.relative_to(ROOT))})
    finally:
        server.terminate()
        server.wait(timeout=10)
        (output / "results.json").write_text(json.dumps(results, indent=2, ensure_ascii=False), encoding="utf-8")
    failed = [result["name"] for result in results if not result["passed"]]
    print(f"\n[UI] {len(results) - len(failed)}/{len(results)} passed; report: {output / 'results.json'}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
