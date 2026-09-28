#!/usr/bin/env python3
"""Join page, CEF and host timing without treating callback counts as screen FPS."""
from collections import Counter
import json
from pathlib import Path
import re
import statistics
import time

ROOT=Path(__file__).resolve().parent
RESULTS=ROOT/'results'

def quantiles(values):
    a=sorted(values)
    def q(p):return round(a[int((len(a)-1)*p)],3) if a else None
    return dict(n=len(a),p50=q(.5),p95=q(.95),p99=q(.99),max=q(1))

def cadence(times):
    a=sorted(times);gaps=[(b-a)*1000 for a,b in zip(a,a[1:])]
    return dict(interval_ms=quantiles(gaps),gaps_over25=sum(x>25 for x in gaps),gaps_over33=sum(x>33.4 for x in gaps))

def main():
    captures=[]
    for path in RESULTS.glob('*.jsonl'):
        rows=[]
        for line in path.read_text().splitlines():
            try:rows.append(json.loads(line))
            except json.JSONDecodeError:pass
        if rows and rows[0].get('kind')=='start':
            stages_path=path.with_name(path.stem+'_stages.json')
            captures.append((path,rows,json.loads(stages_path.read_text()) if stages_path.exists() else []))
    offset=time.time()-time.monotonic()
    reports=[]
    for path in sorted(RESULTS.glob('*_page.json')):
        page=json.loads(path.read_text())
        start,end=page['start_epoch_ms']/1000,page['end_epoch_ms']/1000
        matching=[c for c in captures if c[1][0]['wall']<=start and c[1][-1]['wall']>=end]
        if len(matching)!=1:continue
        capture,rows,stages=matching[0];seconds=end-start
        active=[r for r in rows if start<=r['wall']<end]
        draws=[r['wall'] for r in active if 'J4 draw frame=' in r.get('message','')]
        paints=[r['wall'] for r in active if re.search(r'on_(accelerated_)?paint delivered=',r.get('message',''))]
        samples=[r for r in active if r['kind']=='sample']
        pump=[]
        for r in rows:
            m=re.search(r'call mono_ns=(\d+) gap_us=(\d+) duration_us=(\d+)',r.get('message',''))
            if m and start<=int(m[1])/1e9+offset<end:pump.append((int(m[2])/1000,int(m[3])/1000))
        selected=[s for s in stages if start<=s['start_us']/1e6+offset<end]
        out={'sample':path.stem.removesuffix('_page'),'capture':capture.name,'seconds':round(seconds,3),
             'page':{k:v for k,v in page.items() if k not in ['frames','wheels','scrolls','injection_calls','long_tasks']},
             'page_long_tasks':len(page['long_tasks']),'wheel_events':len(page['wheels']),
             'paint_callbacks_s':round(len(paints)/seconds,2),'draw_callbacks_s':round(len(draws)/seconds,2),
             'draw_log_cadence':cadence(draws),
             'pump_gap_ms':quantiles([x[0] for x in pump]),'pump_duration_ms':quantiles([x[1] for x in pump]),
             'pump_gaps_over50':sum(x[0]>50 for x in pump),
             'cpu_by_role':{role:round(statistics.mean(sum(p['cpu_pct'] for p in s['processes'] if p['role']==role) for s in samples),1)
                            for role in ['main','renderer','gpu-process']} if samples else {},
             'total_cpu':round(statistics.mean(s['cpu_pct'] for s in samples),1) if samples else None,
             'stage_durations_ms':{name:quantiles([(s['end_us']-s['start_us'])/1000 for s in selected if s['stage']==name])
                                   for name in ['paint','upload','render']}}
        rendered=[s['start_us']/1e6 for s in selected if s['stage']=='render']
        if rendered:out['render_callback_cadence']=cadence(rendered)
        presented=sorted({s['data'][1]/1e6 for s in selected if s['stage']=='presentation' and s['data'][1]>0})
        out['presentation_feedback_available']=bool(presented)
        if presented:out['window_presentation_cadence']=cadence(presented)
        damaged=[s['data'][4]*s['data'][5]*4 for s in selected if s['stage']=='paint']
        if damaged:out['paint_damage_bytes']=quantiles(damaged)
        reports.append(out)
    output=RESULTS/'conversation_summary.json'
    output.write_text(json.dumps({'clock_offset_epoch_minus_monotonic':offset,
        'notes':['100% CPU means one core.','Paint/draw counts are callbacks, not screen FPS.',
                 'Log cadence uses receipt times; render_callback_cadence uses in-process timestamps.',
                 'Window presentation feedback may include repeated content and native UI frames.'],
        'runs':reports},indent=2))
    for r in reports:
        print(json.dumps({'run':r['sample'],'draw_s':r['draw_callbacks_s'],
              'page_s':round(r['page']['observed_frames']/r['seconds'],2),
              'page_p95':round(r['page']['frame_intervals_ms']['p95'],2),
              'pump_p95':r['pump_gap_ms']['p95'],'cpu':r['total_cpu'],
              'render_p95':r['stage_durations_ms']['render']['p95'],
              'presentation':r['presentation_feedback_available']}))

if __name__=='__main__':main()
