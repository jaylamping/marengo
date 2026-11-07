#!/usr/bin/env python3
"""Is a review about this packet? Run before reading it. Usage: check_on_target.py <packet dir> <review.md> [more reviews]
Takes the part numbers and net names a review puts in backticks, and on electrical packets its part refs, and looks for
each in the packet's text files. Real refs aren't enough: a made-up review can reuse them with invented values (rev B Argon,
R29 = 30.1 kOhm), so with a netlist in the packet it also compares each "R29 = value" it cites with the netlist.
Exit 1 if any review is under a limit."""
import os,re,sys
LIMIT=0.75; VALUE_LIMIT=0.7; MIN_VALUES=10  # a review's proposed values also get counted, so a few cites prove nothing
TEXT=('.md','.txt','.py','.json','.net','.kicad_sch','.kicad_sym','.kicad_mod','.csv','.rpt','.sh')
REF=re.compile(r'\b(?:RS|TP|[UQRCDJF])\d{1,4}\b')  # no L: reviewers write L120 for a line number
TICK=re.compile(r'`([^`\n]{2,60})`')
MULT={'':1,'R':1,'k':1e3,'K':1e3,'M':1e6,'m':1e-3,'u':1e-6,'µ':1e-6,'μ':1e-6,'n':1e-9,'p':1e-12}
CITED=re.compile(r'\b((?:RS|R|C)\d{1,3})`?\s*(?:=|\(|\s)\s*\$?([0-9]+(?:\.[0-9]+)?)(?:\\text\{)?\s*([kKMmunpµμ]?)(?=[ΩRFHh\\ ]|Ohm)')
def value(s):
    m=re.match(r'([0-9]+(?:\.[0-9]+)?)([kKMmunpµμR]?)',s)
    return float(m[1])*MULT[m[2]] if m else None
def netlist(root):
    out={}
    for d,sub,files in os.walk(root):
        sub[:]=[s for s in sub if s!='reviews']
        for f in files:
            if f.endswith('.net'):
                t=open(os.path.join(d,f),encoding='utf-8',errors='ignore').read()
                for r,v in re.findall(r'\(comp\s*\(ref\s*"([^"]+)"\)\s*\(value\s*"([^"]+)"\)',t): out[r]=value(v)
    return out
def values(t,comps):
    good=bad=0
    for r,a,u in set(CITED.findall(t)):
        if comps.get(r) is None: continue
        if abs(float(a)*MULT[u]-comps[r])<=0.02*comps[r]: good+=1
        else: bad+=1
    return good,bad
def corpus(root):
    out=[]
    for d,sub,files in os.walk(root):
        sub[:]=[s for s in sub if s!='reviews']
        for f in files:
            if f.endswith(TEXT): out.append(open(os.path.join(d,f),encoding='utf-8',errors='ignore').read())
    return '\n'.join(out)
def names(t,electrical):
    # part numbers, net names and ids: mixed letters and digits, or names with an underscore. Prose and numbers say nothing.
    ticks={x.strip() for x in TICK.findall(t)}
    ticks={x for x in ticks if re.fullmatch(r'[A-Za-z+][A-Za-z0-9_+.-]{2,}',x) and re.search(r'[0-9_]',x) and not re.search(r'[.](py|md|txt|json|png|csv)$',x)}
    return ticks|set(REF.findall(t)) if electrical else ticks
def main():
    root=sys.argv[1]; text=corpus(root); low=text.lower(); bad=0; comps=netlist(root)
    electrical='(export' in text or 'kicad_sch' in text
    for p in sys.argv[2:]:
        t=open(p,encoding='utf-8',errors='ignore').read(); n=names(t,electrical)
        if not n: print('skip',os.path.basename(p),': cites no names, read it by hand'); continue
        miss=sorted(x for x in n if x.lower() not in low)
        share=1-len(miss)/len(n); good,wrong=values(t,comps)
        ok=share>=LIMIT and (good+wrong<MIN_VALUES or good/(good+wrong)>=VALUE_LIMIT)
        print(f"{'ok  ' if ok else 'FAIL'} {os.path.basename(p)}: {len(n)-len(miss)} of {len(n)} names found, {share:.0%}; {good} of {good+wrong} cited values match the netlist")
        if miss: print('     not in the packet:',', '.join(miss[:12]))
        bad+=not ok
    sys.exit(1 if bad else 0)
main()
