#!/usr/bin/env python3
"""Collect Chrome stage names/times only; drop all trace arguments and page data."""
import argparse
from collections import Counter
import json
from pathlib import Path
import subprocess
import urllib.request
from cdp_local import Client

ROOT=Path(__file__).resolve().parent

class TraceClient(Client):
    def __init__(self,url):
        super().__init__(url)
        self.events=[]
        self.finished=False
    def receive(self):
        reply=super().receive()
        if reply.get('method')=='Tracing.dataCollected':
            for e in reply['params']['value']:
                # Never retain args: these can contain URLs, DOM data and strings.
                kept={k:e[k] for k in ['cat','name','ph','ts','dur','tdur','pid','tid'] if k in e}
                if e.get('ph')=='M' and e.get('name') in ['process_name','thread_name']:
                    name=e.get('args',{}).get('name','')
                    if name in ['Browser','Renderer','Gpu','CrGpuMain','CrRendererMain','VizCompositorThread','Compositor'] or name.startswith('CompositorTileWorker'):
                        kept['known_thread_name']=name
                self.events.append(kept)
        elif reply.get('method')=='Tracing.tracingComplete':self.finished=True
        return reply

def main():
    p=argparse.ArgumentParser();p.add_argument('label');p.add_argument('--target',default='conversation',choices=['conversation','list'])
    args=p.parse_args()
    if not args.label.replace('_','').replace('-','').isalnum():p.error('Invalid label')
    path=ROOT/'results'/f'{args.label}_trace.json'
    path.parent.mkdir(exist_ok=True)
    if path.exists() or (ROOT/'results'/f'{args.label}_page.json').exists():p.error('Output already exists')
    with urllib.request.urlopen('http://127.0.0.1:9333/json/version') as r:version=json.load(r)
    c=TraceClient(version['webSocketDebuggerUrl'])
    try:
        c.call('Tracing.start',{'categories':'devtools.timeline,cc,gpu,viz,blink,disabled-by-default-devtools.timeline,disabled-by-default-gpu.service',
                               'options':'record-continuously','transferMode':'ReportEvents'})
        run=subprocess.run(['python3',str(ROOT/'conversation_probe.py'),args.label,'--target',args.target])
        c.call('Tracing.end',{})
        while not c.finished:c.receive()
        path.write_text(json.dumps(c.events))
        totals=Counter();counts=Counter();metadata=[]
        for e in c.events:
            if e.get('ph')=='X' and e.get('dur'):
                key=(e['pid'],e['tid'],e['name']);totals[key]+=e['dur'];counts[key]+=1
            if 'known_thread_name' in e:metadata.append(e)
        print(json.dumps({'trace':path.name,'events':len(c.events),'threads':metadata,
            'top_inclusive_durations':[{'pid':k[0],'tid':k[1],'name':k[2],'total_ms':round(v/1000,1),'n':counts[k]} for k,v in totals.most_common(25)]}),flush=True)
        if run.returncode:raise SystemExit(run.returncode)
    finally:c.close()

if __name__=='__main__':main()
