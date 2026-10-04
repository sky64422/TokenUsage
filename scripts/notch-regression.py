"""Native regression against the isolated preview, never the installed app."""
from notch_test_support import prepare_preview, reset_notch
from pathlib import Path
import ctypes, json, os, time
from ctypes import wintypes
import pyautogui
from playwright.sync_api import sync_playwright, expect

u=ctypes.windll.user32
u.GetWindowLongW.argtypes=[wintypes.HWND,ctypes.c_int]
u.GetAncestor.argtypes=[wintypes.HWND,ctypes.c_uint];u.GetAncestor.restype=wintypes.HWND
u.WindowFromPoint.argtypes=[wintypes.POINT];u.WindowFromPoint.restype=wintypes.HWND
with sync_playwright() as p:
 browser=p.chromium.connect_over_cdp('http://127.0.0.1:9223');page=browser.contexts[0].pages[0]
 assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.tokenusage.notch-preview'
 prepare_preview(page)
 def invoke(c,a={}):return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)',[c,a])
 def layout():return invoke('set_notch_placement',{'placement':{'edge':'right','offset':.5,'monitor_hint':None}})
 def reset():reset_notch(page);page.evaluate('()=>document.activeElement?.blur()')
 reset();l=layout()
 # Every pointer path opens, stays open while crossing, and closes when leaving.
 for i in range(20):
  page.locator('[data-id=claude]').dispatch_event('pointerenter')
  expect(page.locator('.notch-detail')).to_be_visible()
  reset();expect(page.locator('.notch-detail')).to_be_hidden()
 # Keyboard: Escape from settings retains pinned provider, next Escape closes it.
 page.locator('[data-id=claude]').evaluate('(e)=>e.click()')
 page.locator('.notch').dispatch_event('contextmenu')
 page.keyboard.press('Escape')
 expect(page.locator('.detail-settings')).to_be_hidden()
 expect(page.locator('.notch-detail')).to_be_visible()
 page.keyboard.press('Escape')
 expect(page.locator('.notch-detail')).to_be_hidden()
 # Disabling the selected provider must leave Settings open.
 page.locator('[data-id=claude]').evaluate('(e)=>e.click()')
 page.locator('.notch').dispatch_event('contextmenu')
 page.locator('.provider-card-btn[data-provider=claude]').evaluate('(e)=>e.click()')
 expect(page.locator('[data-id=claude]')).to_be_hidden()
 expect(page.locator('.detail-settings')).to_be_visible()
 # One-provider geometry and last-enabled lock.
 page.locator('.provider-card-btn[data-provider=codex]').evaluate('(e)=>e.click()')
 expect(page.locator('[data-id=codex]')).to_be_hidden()
 page.locator('.provider-card-btn[data-provider=agy]').evaluate('(e)=>e.click()')
 expect(page.locator('[data-id=agy]')).to_be_hidden()
 expect(page.locator('.provider-card-btn[data-provider=grok]')).to_be_disabled()
 for id in ['claude','codex','agy']:page.locator(f'.provider-card-btn[data-provider={id}]').evaluate('(e)=>e.click()');expect(page.locator(f'[data-id={id}]')).to_be_visible()
 reset()
 # Native gestures: the whole notch moves, but a short ring click still pins.
 assert page.locator('.notch-grip, .notch-settings, .notch-tools').count()==0
 for edge in ['right','top']:
  for on_ring in [False,True]:
   reset()
   l=invoke('set_notch_placement',{'placement':{'edge':edge,'offset':.5,'monitor_hint':None}})
   page.wait_for_function('(e)=>document.querySelector(".notch").dataset.edge===e',arg=edge)
   n=l['notch'];scale=l['scale'];vertical=edge=='right'
   along=l['metrics']['inset']+l['metrics']['cell']/2
   across=36 if on_ring else 6
   x=n['x']+(across if vertical else along)*scale;y=n['y']+(along if vertical else across)*scale
   pyautogui.moveTo(x,y,duration=.2);pyautogui.mouseDown()
   try:
    pyautogui.moveTo(x+(0 if vertical else 80),y+(80 if vertical else 0),duration=.4)
    assert invoke('get_state')['settings']['notch']['offset']==.5,'saved before release'
   finally:pyautogui.mouseUp()
   page.wait_for_function('async()=> (await window.__TAURI__.core.invoke("get_state")).settings.notch.offset > .5')
   expect(page.locator('[data-id=claude]')).to_have_attribute('aria-pressed','false')
 reset();l=layout();n=l['notch'];scale=l['scale']
 x=n['x']+36*scale;y=n['y']+(l['metrics']['inset']+l['metrics']['cell']/2)*scale
 pyautogui.moveTo(x,y,duration=.2);pyautogui.mouseDown()
 try:pyautogui.moveTo(x,y+2,duration=.1)
 finally:pyautogui.mouseUp()
 expect(page.locator('[data-id=claude]')).to_have_attribute('aria-pressed','true')
 assert invoke('get_state')['settings']['notch']['offset']==.5
 pyautogui.click(button='right')
 expect(page.locator('.detail-settings')).to_be_visible()
 pyautogui.click(button='right')
 expect(page.locator('.detail-settings')).to_be_hidden()
 page.keyboard.press('Escape')
 page.locator('.notch').focus();page.keyboard.press('Shift+F10')
 expect(page.locator('.detail-settings')).to_be_visible()
 reset()
 # Move to real negative-coordinate display, then recover a missing monitor hint.
 areas=invoke('get_notch_monitors');negative=next(m for m in areas if m['bounds']['x']<0)
 l=invoke('set_notch_placement',{'placement':{'edge':'left','offset':.25,'monitor_hint':negative['name']}})
 assert l['notch']['x']<0
 l=invoke('set_notch_placement',{'placement':{'edge':'right','offset':.5,'monitor_hint':'missing-display'}})
 assert l['monitor']==areas[0]['name']
 reset();l=layout()
 # A blocked settings file reproduces a write failure during placement save.
 data=Path(page.evaluate('async()=>window.__TAURI__.path.appDataDir()'))
 state=data/'token-usage-state.json';backup=data/'token-usage-state.smoke-backup.json'
 assert not backup.exists()
 committed=invoke('get_state')['settings']['notch']
 state.rename(backup);state.mkdir()
 try:
  try:invoke('set_notch_placement',{'placement':{**committed,'offset':.3}})
  except Exception as e:assert 'write' in str(e).lower() or 'denied' in str(e).lower() or '거부' in str(e)
  else:raise AssertionError('expected failed save')
  assert invoke('get_state')['settings']['notch']==committed
 finally:
  state.rmdir();backup.rename(state)
 # Closed notch corner must again be click-through after that failure.
 n=l['notch'];x=int(n['x']+36);y=int(n['y']+n['height']/2)
 pyautogui.moveTo(x,y,duration=.2)
 inside=u.GetAncestor(u.WindowFromPoint(wintypes.POINT(x,y)),2)
 pyautogui.moveTo(1000,400,duration=.2)
 deadline=time.monotonic()+2
 while time.monotonic()<deadline:
  if u.GetWindowLongW(inside,-20)&0x20:break
 assert u.GetWindowLongW(inside,-20)&0x20,'drag capture survived failed save'
 page.emulate_media(reduced_motion='reduce')
 page.locator('[data-id=claude]').dispatch_event('pointerenter')
 assert page.locator('.notch-detail').evaluate('(e)=>getComputedStyle(e).animationName')=='none'
 page.emulate_media(reduced_motion='no-preference');reset()
 print(json.dumps({'whole_notch_drag_both_axes':True,'ring_click_threshold':True,'native_right_click_settings':True,'keyboard_settings':True,'repeat_open_close':20,'pin_escape':True,'provider_disable_keeps_settings':True,'one_provider_lock':True,'negative_monitor':True,'missing_monitor_fallback':True,'failed_drag_save_recovers':True,'reduced_motion':True}))
