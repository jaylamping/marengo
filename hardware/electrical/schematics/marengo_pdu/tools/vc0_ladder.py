# VC0 excursion in a VSYS short, cell ladder model. Run: uv run --no-project --with numpy python tools/vc0_ladder.py
# VC0..VC13 ladder: each VCn 100R to tap, 220nF between adjacent, C23 VC0-VSS, D15, tap n = -Rret*i + n*(3.7-rc*i)
def sim(Rret,rc,toff,C13=.22e-6,C23=1e-6,Vf10=.38,diode=True,dI=False,Lp=2e-6,Rp=.09,Cs=320e-6):
    N=14; C=[.22e-6]*(N-1); C[0]=C13; R=[100.0]*N; R[13]=20.0
    dt=4e-9; steps=int(320e-6/dt); i=0;vs=0.0
    tap=lambda n,i:-Rret*i+n*(3.7-rc*i)
    v=[tap(n,0) for n in range(N)]; vmin=0;idm=0
    # capacitor charge states: solve by tridiagonal each step for node voltages from currents
    import numpy as np
    A=np.zeros((N,N))
    for k in range(N-1):
        A[k,k]+=C[k];A[k+1,k+1]+=C[k];A[k,k+1]-=C[k];A[k+1,k]-=C[k]
    A[0,0]+=C23
    Ai=np.linalg.inv(A); v=np.array(v); t=0
    for s in range(steps):
        if t<toff: vs=i*.002
        else:
            itv=max(0,(vs-64.4)/.91); vs+=(i-itv)/Cs*dt
        i+=(54.6-Rp*i-vs)/Lp*dt
        if t>=toff and i<0:i=0
        I=np.array([(tap(n,i)-v[n])/R[n] for n in range(N)])
        if diode and v[0]<0: 
            d=.01*10**((-v[0]-Vf10)/.06); I[0]+=d; idm=max(idm,d)
        v=v+Ai@I*dt; vmin=min(vmin,v[0]); t+=dt
    return round(vmin,3),round(idm*1e3,1)
if __name__=='__main__':
    for lab,vf in (('25 C',.26),('0 C',.31),('-40 C',.38)):
        print(lab,'FET off 83 us, return 1.0 mOhm, 3 mOhm per cell, C13 100 nF:',sim(1e-3,3e-3,83e-6,C13=.1e-6,Vf10=vf))
