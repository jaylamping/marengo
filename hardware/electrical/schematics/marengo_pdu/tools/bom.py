import sys,csv,re
sys.path.insert(0,sys.argv[1]); from sx import *
N=parse(open(sys.argv[2]).read())
G={}
for c in getall(get(N,'components'),'comp'):
    r=str(get(c,'ref')[1]); v=str(get(c,'value')[1]); fp=get(c,'footprint'); fp=str(fp[1]) if fp else ''
    d=any(str(get(p,'name')[1])=='dnp' for p in getall(c,'property'))
    G.setdefault((v,fp,d),[]).append(r)
key=lambda s:(re.match(r'[A-Za-z#]+',s).group(),int(re.search(r'\d+',s).group()))
rows=sorted(G.items(),key=lambda kv:key(sorted(kv[1],key=key)[0]))
w=csv.writer(open(sys.argv[3],'w',newline=''),quoting=csv.QUOTE_ALL,lineterminator='\n')
w.writerow(['Refs','Value','Footprint','Qty','DNP'])
for (v,fp,d),rs in rows: rs=sorted(rs,key=key); w.writerow([','.join(rs),v,fp,len(rs),'DNP' if d else ''])
print(len(rows),'lines',sum(len(x) for x in G.values()),'parts')
