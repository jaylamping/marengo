#!/usr/bin/env python3
"""Cost of an `opencode run --format json` stream so far. Usage: oc_cost.py <run.json> <in $/M> <cache read $/M> <out $/M> [cache write $/M]
Sums the per-step token counts in step_finish events. Reasoning tokens bill as output. Prints dollars."""
import json,sys
p,pi,pc,po=sys.argv[1],*map(float,sys.argv[2:5]); pw=float(sys.argv[5]) if len(sys.argv)>5 else pi
c=0.0
for line in open(p,encoding='utf-8',errors='ignore'):
    try: e=json.loads(line)
    except ValueError: continue
    t=(e.get('part') or {}).get('tokens') if isinstance(e,dict) else None
    if not t: continue
    k=t.get('cache') or {}
    c+=(t.get('input',0)*pi+k.get('read',0)*pc+k.get('write',0)*pw+(t.get('output',0)+t.get('reasoning',0))*po)/1e6
print(f"{c:.4f}")
