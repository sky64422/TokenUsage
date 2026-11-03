"""Restart the isolated preview in GDI/WGC/WGC/GDI order; keep renderer identical."""
import json
import argparse
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'tmp' / 'liquid-backend-comparison'
OUT.mkdir(parents=True, exist_ok=True)
CONFIG = ROOT / 'tmp' / 'notch-wgc-preview.json'
CONFIG.write_text(json.dumps({'identifier': 'com.tokenusage.notch-preview',
    'build': {'beforeDevCommand': '', 'devUrl': 'http://localhost:1524'}}))
CREATE_NO_WINDOW = 0x08000000
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--raw-gpu', action='store_true', help='Separate GDI/WGC GPU sampling run, without claiming ABBA CPU comparability')
args = parser.parse_args()

def cdp_ready():
    try:
        with urllib.request.urlopen('http://127.0.0.1:9223/json/version', timeout=.3):
            return True
    except (OSError, TimeoutError):
        return False

if cdp_ready():
    raise SystemExit('Close the existing CDP 9223 preview before comparison.')

for index, backend in enumerate(['gdi', 'wgc'] if args.raw_gpu else ['gdi', 'wgc', 'wgc', 'gdi'], 1):
    label = f'{backend}-gpu-{index}' if args.raw_gpu else f'{backend}-{index}'
    env = os.environ.copy()
    for name in ['TOKENUSAGE_WGC_PREVIEW', 'TOKENUSAGE_WGC_BORDERLESS_PREVIEW', 'TOKENUSAGE_CAPTURE_DEBUG']:
        env.pop(name, None)
    env.update(TOKENUSAGE_SKIP_DIRECT_QUOTA='1',
        WEBVIEW2_USER_DATA_FOLDER=str(ROOT / 'tmp' / 'notch-webview'),
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9223')
    if backend == 'wgc':
        env.update(TOKENUSAGE_WGC_PREVIEW='1', TOKENUSAGE_WGC_BORDERLESS_PREVIEW='1')
    else:
        env.update(TOKENUSAGE_WGC_PREVIEW='0')
    with (OUT / f'{label}-preview.log').open('w') as log:
        process = subprocess.Popen(['cmd', '/c', 'npm', 'run', 'tauri', 'dev', '--', '--no-watch',
            '--config', str(CONFIG)], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
            creationflags=CREATE_NO_WINDOW)
        try:
            deadline = time.monotonic() + 120
            while not cdp_ready():
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError(f'{label} preview did not start; see {log.name}')
                time.sleep(.2)
            print(f'Running {label}', flush=True)
            subprocess.run([sys.executable, '-X', 'utf8', 'scripts/liquid-native-benchmark.py',
                '--backend-label', label, '--seconds', '8', '--preview-owner-pid', str(process.pid)] + (['--raw-gpu'] if args.raw_gpu else []), cwd=ROOT, check=True)
        finally:
            if process.poll() is None:
                subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=CREATE_NO_WINDOW)
                process.wait(timeout=15)
            deadline = time.monotonic() + 10
            while cdp_ready() and time.monotonic() < deadline:
                time.sleep(.2)
            if cdp_ready():
                raise RuntimeError('Owned preview did not close; refusing to attach next run.')
