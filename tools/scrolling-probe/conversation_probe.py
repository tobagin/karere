#!/usr/bin/env python3
"""Drive the current diagnostic page and save timing/geometry, never content."""
import argparse
import json
from pathlib import Path
import time
import urllib.parse
import urllib.request
from cdp_local import Client

ROOT=Path(__file__).resolve().parent

def connect():
    with urllib.request.urlopen('http://127.0.0.1:9333/json/list',timeout=3) as r:
        targets=json.load(r)
    for t in targets:
        if t.get('type')=='page' and (urllib.parse.urlparse(t.get('url','')).hostname=='web.whatsapp.com'
                                    or t.get('url','').startswith('data:')):
            c=Client(t['webSocketDebuggerUrl'])
            if c.evaluate("document.visibilityState === 'visible'"):
                return c
            c.close()
    raise RuntimeError('Visible diagnostic page unavailable')

def metrics(c):
    allow={'LayoutCount','RecalcStyleCount','LayoutDuration','RecalcStyleDuration','ScriptDuration',
           'TaskDuration','TaskOtherDuration','DevToolsCommandDuration','V8CompileDuration','Timestamp'}
    return {m['name']:m['value'] for m in c.call('Performance.getMetrics',{})['metrics'] if m['name'] in allow}

def main():
    p=argparse.ArgumentParser()
    p.add_argument('label')
    p.add_argument('--target',choices=['conversation','list'],default='conversation')
    p.add_argument('--mode',choices=['programmatic','wheel','observe'],default='programmatic')
    p.add_argument('--duration',type=int,default=20)
    p.add_argument('--wheel-hz',type=float,default=8)
    p.add_argument('--anchor-bottom',type=float)
    args=p.parse_args()
    if not args.label.replace('_','').replace('-','').isalnum(): p.error('Invalid label')
    if not 1<=args.duration<=25: p.error('--duration must be 1 to 25 seconds (CDP timeout is 35 seconds)')
    if not 0<args.wheel_hz<=120: p.error('--wheel-hz must be greater than 0 and at most 120')
    path=ROOT/'results'/f'{args.label}_page.json'
    path.parent.mkdir(exist_ok=True)
    if path.exists(): p.error('Output already exists')
    c=connect()
    try:
        c.call('Performance.enable',{'timeDomain':'threadTicks'})
        c.evaluate((ROOT/'conversation_probe.js').read_text())
        before=metrics(c)
        params={'target':args.target,'mode':'observe' if args.mode in ('wheel','observe') else args.mode,
                'warmup':5000,'duration':args.duration*1000,'anchorBottom':args.anchor_bottom}
        geometry=c.evaluate('window.__karereConversationProbe.start('+json.dumps(params)+')')
        host_start=time.monotonic()
        deliveries=[]
        if args.mode=='wheel':
            # Browser input injection; this deliberately bypasses GTK and the physical device.
            interval=1/args.wheel_hz
            while True:
                age=time.monotonic()-host_start
                if age>=args.duration+5:break
                direction=1 if int(age/3)%2==0 else -1
                sent=time.monotonic()
                c.call('Input.dispatchMouseEvent',{'type':'mouseWheel',
                       'x':geometry['x']+geometry['width']/2,'y':geometry['y']+geometry['height']/2,
                       'deltaX':0,'deltaY':direction*300*interval})
                deliveries.append([sent,time.monotonic()])
                time.sleep(max(0,interval-(time.monotonic()-sent)))
        result=c.evaluate('window.__karereConversationProbe.done',await_promise=True)
        after=metrics(c)
        result.update(label=args.label,input_mode=args.mode,host_start_monotonic=host_start,
                      performance_delta={k:after[k]-before[k] for k in before if k in after},
                      injection_calls=deliveries)
        path.write_text(json.dumps(result))
        summary={k:v for k,v in result.items() if k not in ['frames','wheels','scrolls','injection_calls','long_tasks']}
        summary['wheel_events']=len(result['wheels']);summary['long_task_count']=len(result['long_tasks'])
        print(json.dumps(summary),flush=True)
    finally:
        c.close()

if __name__=='__main__':main()
