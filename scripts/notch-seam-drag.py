"""Repeated native seam crossings, including samples that skip the entry band."""
import json
from pathlib import Path
import pyautogui
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9223');page=browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.tokenusage.notch-preview'
    def invoke(c,a={}):return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)',[c,a])
    areas=invoke('get_notch_monitors')
    left=next(m for m in areas if m['bounds']['x']<0);right=areas[0]
    seam=right['bounds']['x'];assert left['bounds']['x']+left['bounds']['width']==seam
    y=max(left['work']['y'],right['work']['y'])+600
    page.evaluate('async()=>{window.seamEvents=[];await window.__TAURI__.event.listen("notch-layout",e=>window.seamLayout=e.payload);await window.__TAURI__.event.listen("notch-drag",e=>window.seamEvents.push(e.payload));}')
    def start(m,edge):
        page.locator('.detail-close').evaluate('(e)=>e.click()');page.evaluate('()=>document.activeElement?.blur()')
        pyautogui.moveTo(seam+600,y,duration=.1)
        l=invoke('set_notch_placement',{'placement':{'edge':edge,'offset':.5,'monitor_hint':m['name']}})
        pyautogui.moveTo(l['notch']['x']+36*l['scale'],l['notch']['y']+102*l['scale'],duration=.2)
        page.evaluate('()=>window.seamEvents=[]');pyautogui.mouseDown()
    def cross(m,edge,speed):
        x=seam+(180 if m==right else -180)*m['scale']
        pyautogui.moveTo(x,y,duration=speed)
        page.wait_for_function('([name,edge])=>window.seamLayout?.monitor===name && window.seamLayout?.edge===edge',arg=[m['name'],edge],timeout=3000)
        assert not page.evaluate('()=>window.seamEvents.some(e=>e.finished)'), 'drag ended during crossing'
        page.screenshot(path=f'tmp/seam-{edge}.png',omit_background=True)
    start(left,'right')
    try:
        for speed in [0.,.11,.5,0.,.11]:
            cross(right,'left',speed);cross(left,'right',speed)
    finally:pyautogui.mouseUp()
    page.wait_for_function('()=>window.seamEvents.some(e=>e.finished)')
    assert invoke('get_state')['settings']['notch']['monitor_hint']==left['name']
    # New drags after releasing must work in both directions as well.
    for i in range(6):
        source,target=(left,right) if i%2==0 else (right,left)
        target_edge='left' if target==right else 'right'
        start(source,'right' if source==left else 'left')
        try:cross(target,target_edge,0.)
        finally:pyautogui.mouseUp()
        page.wait_for_function('()=>window.seamEvents.some(e=>e.finished)')
        saved=invoke('get_state')['settings']['notch']
        assert saved['monitor_hint']==target['name'] and saved['edge']==target_edge
    invoke('set_notch_placement',{'placement':{'edge':'right','offset':.5,'monitor_hint':right['name']}})
    print(json.dumps({'continuous_crossings':10,'separate_drags':6,'speeds_seconds':[0,.11,.5],'both_directions':True,'persisted_target_correct':True}))
