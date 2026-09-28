#!/usr/bin/env python3
import argparse,json,subprocess,time
from pathlib import Path
from conversation_probe import connect

ROOT=Path(__file__).resolve().parent

def main():
    p=argparse.ArgumentParser();p.add_argument('prefix');p.add_argument('--mode',default='programmatic',choices=['programmatic','wheel'])
    p.add_argument('--production',action='store_true',help='Toggle the compiled wallpaper stylesheet instead of an inline hint')
    args=p.parse_args()
    c=connect()
    c.evaluate((ROOT/'wallpaper_probe.js').read_text())
    production="document.getElementById('karere-conversation-wallpaper').sheet"
    original_disabled=c.evaluate(production+'.disabled') if args.production else None
    try:
        for enabled,suffix in [(False,'original_1'),(True,'layer_1'),(True,'layer_2'),(False,'original_2')]:
            if args.production:
                c.evaluate(production+'.disabled='+str(not enabled).lower())
                actual=c.evaluate('getComputedStyle(window.__karereWallpaperProbe.el).willChange')
                if actual!=('transform' if enabled else 'auto'):
                    raise RuntimeError('Production stylesheet did not switch the background as expected')
            else:
                actual=c.evaluate('window.__karereWallpaperProbe.set('+str(enabled).lower()+')')
            time.sleep(3)
            print(json.dumps({'condition':actual,'sample':suffix}),flush=True)
            subprocess.run(['python3',str(ROOT/'conversation_probe.py'),args.prefix+'_'+suffix,
                            '--anchor-bottom','3452.5','--mode',args.mode],check=True)
    finally:
        if args.production:c.evaluate(production+'.disabled='+str(original_disabled).lower())
        else:c.evaluate('window.__karereWallpaperProbe.set(false)')
        c.close()

if __name__=='__main__':main()
