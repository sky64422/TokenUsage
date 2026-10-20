"""Recorded token UI across providers; synthetic metadata and no native/provider reads."""
import argparse, json
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--port', type=int, default=1523)
args = parser.parse_args()
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width':400,'height':2000}, reduced_motion='reduce')
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.route('**/provider-token-details-smoke', lambda route: route.fulfill(content_type='text/html',
        body='<html><body><div id="rows" style="width:260px"></div></body></html>'))
    page.goto(f'http://localhost:{args.port}/provider-token-details-smoke')
    page.evaluate('''async()=>{
      await import('/src/styles/fonts.css');await import('/src/styles/tokens.css');
      await import('/src/styles/app.css');await import('/src/styles/notch.css');
      const {mountProviders}=await import('/src/ui/providers.ts');
      window.ui=mountProviders(document.querySelector('#rows'));
      const reset='2099-10-18T05:00:00Z';
      window.period=(model,input)=>{
        const counts={input,cache_read:input/2,cache_write:null,output:0,reasoning:null};
        return {models:[{model,counts}],totals:counts};
      };
      window.tokens=(model)=>({source:'local_records',status:'ready',as_of:'fixture',
        today:window.period(model,100),week:window.period(model,200),month:window.period(model,300)});
      const quota={kind:'rolling_5h',label:'5h',used:40,limit:100,unit:'percent',used_percent:40,resets_at:reset};
      window.snaps=['claude','codex','grok','agy'].map(provider_id=>({provider_id,display_name:provider_id,
        status:'ok',source:'vendor',message:null,as_of:'fixture',primary_used_percent:40,primary_resets_at:reset,
        windows:[{...quota}],token_usage:provider_id==='agy'?null:window.tokens(provider_id+'-model')}));
      window.snaps[0].windows=['Opus week','Sonnet week'].map(label=>({...quota,kind:'weekly',label}));
      window.snaps[0].token_usage.windows=['Sonnet week','Opus week'].map(label=>({kind:'weekly',resets_at:reset,
        group:null,label,usage:window.period(label==='Opus week'?'claude-opus-test':'claude-sonnet-test',200)}));
      window.snaps[2].token_usage.windows=[{kind:'rolling_5h',resets_at:reset,group:null,label:'5h',usage:window.period('grok-test',100)}];
      window.snaps[3].windows=['Gemini','Claude'].map(group=>({...quota,group}));
      window.publish=()=>window.ui.setSnapshots(structuredClone(window.snaps));
      window.publish();await document.fonts.ready;
    }''')
    def entry(provider):
        return page.locator(f'.provider-entry:has(.provider-card[data-provider="{provider}"])')
    assert page.locator('.token-details-root').count() == 3, 'Every provider with recorded metadata needs its own detail'
    for provider in ['claude','codex','grok']:
        entry(provider).locator('.token-details > summary').click()
    codex_toggle = entry('codex').locator('.token-details > summary')
    expect(entry('codex').locator('.token-overview')).to_contain_text('300')
    expect(entry('grok').locator('.token-overview')).to_contain_text('100')
    codex_toggle.focus()
    page.evaluate('''()=>{
      window.codexPanel=document.querySelector('.provider-card[data-provider=codex]').closest('.provider-entry').querySelector('details');
      window.snaps[3].token_usage=window.tokens(null);
      window.snaps[3].token_usage.windows=[{kind:'rolling_5h',resets_at:window.snaps[3].windows[0].resets_at,
        group:null,label:'5h',usage:window.period(null,100)}];
      window.publish();
    }''')
    assert page.locator('.token-details-root').count() == 4
    expect(codex_toggle).to_be_focused()
    assert page.evaluate('window.codexPanel===document.querySelector(".provider-card[data-provider=codex]").closest(".provider-entry").querySelector("details")')
    ids = page.locator('[id]').evaluate_all('(els)=>els.map(el=>el.id)')
    assert len(ids) == len(set(ids)), ids
    entry('agy').locator('.token-details > summary').click()
    expect(entry('agy').locator('.token-overview')).to_contain_text('50%')
    expect(entry('agy').locator('.token-overview tbody tr').nth(1)).to_contain_text('—')
    assert entry('agy').locator('.track-models').count() == 0, 'Unknown AGY models cannot be assigned to quota families'
    assert entry('claude').locator('.usage-row').nth(0).locator('.token-legend-item').get_attribute('title') == 'claude-opus-test'
    assert entry('claude').locator('.usage-row').nth(1).locator('.token-legend-item').get_attribute('title') == 'claude-sonnet-test'
    assert entry('grok').locator('.track-models').count() == 1, 'Absent/null groups normalize equally'
    assert entry('grok').locator('.track-fill').evaluate('(el)=>el.style.width') == '40%'
    # Same reset and kind must not cross-map AGY family groups.
    page.evaluate('''()=>{
      window.snaps[3].token_usage.windows=['Claude','Gemini'].map(group=>({group,kind:'rolling_5h',label:'5h',
        resets_at:window.snaps[3].windows[0].resets_at,usage:window.period(group+'-test',100)}));
      window.publish();
    }''')
    assert entry('agy').locator('.usage-row').nth(0).locator('.token-legend-item').get_attribute('title') == 'Gemini-test'
    assert entry('agy').locator('.usage-row').nth(1).locator('.token-legend-item').get_attribute('title') == 'Claude-test'
    # An ambiguous legacy weekly summary must not shade both Claude buckets.
    page.evaluate('''()=>{
      window.snaps[0].token_usage.windows=[{kind:'weekly',resets_at:window.snaps[0].windows[0].resets_at,usage:window.period('legacy',100)}];
      window.snaps[2].windows[0].resets_at='2099-10-18T10:00:00Z';
      window.snaps[1].token_usage.month=window.period('codex-model',900);
      window.snaps[1].token_usage.as_of='refresh';window.publish();
    }''')
    assert entry('claude').locator('.track-models').count() == 0
    assert entry('grok').locator('.track-models').count() == 0
    expect(entry('codex').locator('.token-details')).to_have_attribute('open','')
    expect(entry('codex').locator('.token-overview')).to_contain_text('900')
    expect(entry('grok').locator('.token-overview')).to_contain_text('100')
    assert page.locator('.token-provenance,.token-status,.token-note,.token-legend-percent').count() == 0
    page.screenshot(path=str(ROOT/'tmp/provider-token-details.png'),full_page=True)
    assert not errors, errors
    result={'providers':4,'late_arrival':True,'unique_aria_ids':True,'independent_summaries_and_refresh':True,
            'unknown_fields_dash':True,'group_and_label_collisions_isolated':True,'vendor_quota_width_preserved':True}
    (ROOT/'tmp/provider-token-details-results.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result));browser.close()
