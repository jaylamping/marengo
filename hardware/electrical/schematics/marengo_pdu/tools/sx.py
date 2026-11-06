import re
TOK=re.compile(r'\s*(\(|\)|"(?:[^"\\]|\\.)*"|[^\s()]+)')
class S(str): pass
def parse(t):
    st=[];cur=[];pos=0;n=len(t)
    while True:
        m=TOK.match(t,pos)
        if not m: break
        pos=m.end();k=m.group(1)
        if k=='(':
            st.append(cur);cur=[]
        elif k==')':
            d=cur;cur=st.pop();cur.append(d)
        elif k[0]=='"':
            cur.append(S(k[1:-1].replace('\\"','"').replace('\\n','\n').replace('\\\\','\\')))
        else: cur.append(k)
    return cur[0]
def q(s): return '"'+s.replace('\\','\\\\').replace('"','\\"').replace('\n','\\n')+'"'
def dump(x,ind=0):
    if isinstance(x,S): return q(x)
    if not isinstance(x,list): return str(x)
    if all(not isinstance(i,list) for i in x): return '('+' '.join(dump(i) for i in x)+')'
    p='\t'*(ind+1); out='('
    first=True; 
    for i in x:
        if isinstance(i,list): out+='\n'+p+dump(i,ind+1)
        else: out+=(' ' if not first else '')+dump(i)
        first=False
    return out+'\n'+'\t'*ind+')'
def get(x,k): 
    for i in x:
        if isinstance(i,list) and i and i[0]==k: return i
def getall(x,k): return [i for i in x if isinstance(i,list) and i and i[0]==k]
