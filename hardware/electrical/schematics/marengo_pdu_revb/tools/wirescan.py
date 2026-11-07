import re,sys,glob,os
D=sys.argv[1]
for f in sorted(glob.glob(D+'/*.kicad_sch')):
    t=open(f).read()
    W=[((float(a),float(b)),(float(c),float(d))) for a,b,c,d in re.findall(r'\(wire\s*\(pts\s*\(xy ([\d.-]+) ([\d.-]+)\)\s*\(xy ([\d.-]+) ([\d.-]+)\)',t)]
    L=[(m[1],(float(m[2]),float(m[3]))) for m in re.finditer(r'\((?:global_label|label|hierarchical_label) "([^"]+)"[^(]*(?:\(shape \w+\)\s*)?\(at ([\d.-]+) ([\d.-]+)',t) for m in [(0,)+m.groups()]]
    L+=[(m.group(1),(float(m.group(2)),float(m.group(3)))) for m in re.finditer(r'\(symbol\s*\(lib_id "power:(\w+)"\)\s*\(at ([\d.-]+) ([\d.-]+)',t)]
    def on(p,w):
        (a,b),(c,d)=w; e=1e-3
        return min(a,c)-e<=p[0]<=max(a,c)+e and min(b,d)-e<=p[1]<=max(b,d)+e and abs((c-a)*(p[1]-b)-(d-b)*(p[0]-a))<1e-2
    par=list(range(len(W)))
    def fd(x):
        while par[x]!=x: par[x]=par[par[x]]; x=par[x]
        return x
    for i,w in enumerate(W):
        for j,v in enumerate(W):
            if i!=j and (on(w[0],v) or on(w[1],v)): par[fd(i)]=fd(j)
    S={}
    for n,p in L:
        for i,w in enumerate(W):
            if on(p,w): S.setdefault(fd(i),set()).add(n)
    bad={k:v for k,v in S.items() if len(v)>1}
    for k,v in bad.items(): print(os.path.basename(f),v,[W[i] for i in range(len(W)) if fd(i)==k][:4])
