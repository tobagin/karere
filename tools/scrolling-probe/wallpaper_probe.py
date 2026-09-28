#!/usr/bin/env python3
import argparse,json
from pathlib import Path
from conversation_probe import connect

def main():
    p=argparse.ArgumentParser();p.add_argument('state',choices=['on','off']);args=p.parse_args()
    c=connect()
    try:
        c.evaluate(Path(__file__).with_suffix('.js').read_text())
        value=c.evaluate('window.__karereWallpaperProbe.set('+str(args.state=='on').lower()+')')
        print(json.dumps({'will_change':value,'geometry':c.evaluate('window.__karereWallpaperProbe.geometry')}))
    finally:c.close()

if __name__=='__main__':main()
