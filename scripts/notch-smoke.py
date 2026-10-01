"""Optional Windows/WebView2 smoke; see docs/windows-dev.md. Synthetic data only."""
import ctypes,json
from ctypes import wintypes
from pathlib import Path
import pyautogui
from PIL import ImageGrab
from playwright.sync_api import sync_playwright,expect

u=ctypes.windll.user32
u.GetForegroundWindow.restype=wintypes.HWND
u.WindowFromPoint.argtypes=[wintypes.POINT];u.WindowFromPoint.restype=wintypes.HWND
fixtures=[]
for id,name,pct,week in [('claude','Claude',73,7),('codex','Codex',21,12),('grok','Grok',52,None)]:
 windows=[dict(kind='weekly' if id=='grok' else 'rolling_5h',used=pct,limit=100,unit='percent',resets_at='2026-10-01T12:00:00Z',used_percent=pct,label='Week' if id=='grok' else '5h')]
 if week is not None:windows.append(dict(kind='weekly',used=week,limit=100,unit='percent',resets_at='2026-10-06T12:00:00Z',used_percent=week,label='Week'))
 fixtures.append(dict(provider_id=id,display_name=name,windows=windows,status='ok',source='vendor',as_of='2026-09-30T12:00:00Z',message=None,primary_resets_at=windows[0]['resets_at'],primary_used_percent=pct))

with sync_playwright() as p:
 browser=p.chromium.connect_over_cdp('http://127.0.0.1:9223');page=browser.contexts[0].pages[0]
 assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.tokenusage.notch-preview', 'Use the isolated preview identifier'
 errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
 def invoke(cmd,args={}):return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)',[cmd,args])
 def fixture():page.evaluate('async s=>window.__TAURI__.event.emit("snapshots-updated",s)',fixtures)
 def reset():
  page.locator('.detail-close').evaluate('(e)=>e.click()')
  page.evaluate('()=>document.activeElement?.blur()')
 pyautogui.moveTo(1000,500)
 fixture()
 for edge in ['right','left','top','bottom']:
  reset()
  l=invoke('set_notch_placement',dict(placement=dict(edge=edge,offset=.5,monitor_hint=None)))
  page.wait_for_function('(e)=>document.querySelector(".notch").dataset.edge===e',arg=l['edge'])
  page.screenshot(path=f'tmp/notch-{edge}.png',omit_background=True)
  page.locator('[data-id=claude]').dispatch_event('pointerenter')
  expect(page.locator('.notch-detail')).to_be_visible()
  page.wait_for_function('()=>innerWidth>100')
  fixture()
  page.screenshot(path=f'tmp/notch-{edge}-detail.png',omit_background=True)
 reset();l=invoke('set_notch_placement',dict(placement=dict(edge='right',offset=.5,monitor_hint=None)))
 fixture()
 # Native pointer hover must not activate the widget.
 before=u.GetForegroundWindow()
 n=l['notch'];pyautogui.moveTo(n['x']+36,n['y']+l['metrics']['inset']+20,duration=.2)
 expect(page.locator('.notch-detail')).to_be_visible()
 after=u.GetForegroundWindow()
 assert before==after,('hover stole focus',before,after)
 # Native move across the corridor into the actual detail must keep it open.
 l=invoke('set_notch_placement',dict(placement=dict(edge='right',offset=.5,monitor_hint=None)))
 d=l['detail'];pyautogui.moveTo(d['x']+d['width']-20,d['y']+50,duration=.3)
 expect(page.locator('.notch-detail')).to_be_visible()
 # Corners must target a different window from the painted body.
 pyautogui.moveTo(n['x']+36,n['y']+100,duration=.2)
 page.wait_for_function('()=>document.querySelector(".notch-detail").hidden===false')
 inside=u.WindowFromPoint(wintypes.POINT(int(n['x']+36),int(n['y']+100)))
 pyautogui.moveTo(n['x']+1,n['y']+1,duration=.2)
 # Wait on the observable HWND hit result, not a fixed sleep.
 import time
 deadline=time.monotonic()+2
 while time.monotonic()<deadline:
  outside=u.WindowFromPoint(wintypes.POINT(int(n['x']+1),int(n['y']+1)))
  if outside!=inside:break
 assert outside!=inside,'transparent corner intercepted input'
 page.locator('.notch').dispatch_event('contextmenu')
 expect(page.locator('#edge')).to_be_visible()
 page.screenshot(path='tmp/notch-settings.png',omit_background=True)
 print(json.dumps(dict(errors=errors,hover_focus_preserved=before==after,transparent_target_changed=inside!=outside,actual_bottom=l['edge'])))
