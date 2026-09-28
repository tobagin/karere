#!/usr/bin/env python3
"""Settle an open conversation's loaded history without reading its contents."""

import argparse
import json
import time

from probe_common import connect, interrupt_cleanup, target_argument


def main():
    """Load history only in the selected page, opening a chat only when requested."""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--open-row",
        type=int,
        help="Explicitly allow opening this zero-based chat-list row if no chat is open",
    )
    target_argument(parser)
    args = parser.parse_args()
    if args.open_row is not None and args.open_row < 0:
        parser.error("--open-row must be nonnegative")
    c = connect(args.target_id)
    try:
        if not c.evaluate("!!document.querySelector('#main')"):
            if args.open_row is None:
                raise RuntimeError(
                    "Open a conversation first, or explicitly choose --open-row"
                )
            point = c.evaluate(
                "(()=>{let e=[...document.querySelectorAll('#pane-side [role=row]')]["
                + str(args.open_row)
                + "];if(!e)return null;let r=e.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()"
            )
            if not point:
                raise RuntimeError("Original chat row position unavailable")
            for event in ["mousePressed", "mouseReleased"]:
                c.call(
                    "Input.dispatchMouseEvent",
                    dict(type=event, button="left", clickCount=1, **point),
                )
        time.sleep(3)
        c.evaluate(
            "window.__karerePane=[...document.querySelectorAll('#main *')].find(e=>e.clientHeight>100&&e.scrollHeight>e.clientHeight+300&&/auto|scroll/.test(getComputedStyle(e).overflowY));!!window.__karerePane"
        )
        for _ in range(4):
            state = c.evaluate(
                "(()=>{let e=window.__karerePane;return{height:e.clientHeight,scroll_height:e.scrollHeight,top:e.scrollTop}})()"
            )
            if state["scroll_height"] - state["height"] >= 4852.5:
                break
            c.evaluate("window.__karerePane.scrollTo({top:0,behavior:'instant'});true")
            time.sleep(5)
        else:
            raise RuntimeError("Could not load matching history range")
        c.evaluate(
            "window.__karerePane.scrollTo({top:window.__karerePane.scrollHeight-window.__karerePane.clientHeight-3452.5,behavior:'instant'});true"
        )
        time.sleep(3)
        print(
            json.dumps(
                c.evaluate(
                    "(()=>{let e=window.__karerePane;return{height:e.clientHeight,scroll_height:e.scrollHeight,top:e.scrollTop,images:document.querySelector('#main').querySelectorAll('img').length}})()"
                )
            )
        )
    finally:
        c.close()


if __name__ == "__main__":
    with interrupt_cleanup():
        main()
