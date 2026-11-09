#!/usr/bin/env python3
"""List the nets still open after routing, for hand finishing in KiCad: from kicad-cli's DRC JSON (unconnected items),
one row per net with its rule class (tools/domains.py), the number of open connections and the parts involved.
Run with plain Python: python tools/open_nets.py <drc.json> <open_nets.csv>"""
import collections,csv,json,os,re,sys
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__))); import domains
d=json.load(open(sys.argv[1])); n=collections.Counter(); parts=collections.defaultdict(set)
for u in d['unconnected_items']:
    for it in u['items']:
        m=re.search(r'\[([^\]]+)\]',it['description']); net=m.group(1) if m else '?'
        r=re.search(r' of (\S+)',it['description'])
        if r: parts[net].add(r.group(1))
    n[net]+=1
def cls(x): return 'ESTOP (1.5 mm to 54 V)' if domains.is_estop(x) else '54 V' if domains.is_hv(x) else 'rides 54 V' if domains.rides(x) else 'GND' if x=='GND' else 'signal'
with open(sys.argv[2],'w',newline='') as f:
    w=csv.writer(f); w.writerow(['net','class','open connections','parts'])
    for net,k in sorted(n.items(),key=lambda a:(-a[1],a[0])): w.writerow([net,cls(net),k,' '.join(sorted(parts[net]))])
print('open nets',len(n),'connections',sum(n.values()))
