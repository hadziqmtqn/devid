import os, sys, time, signal, subprocess
pgid = int(sys.argv[1])
def members(): 
    out = subprocess.run(['ps', '-axo', 'pid=,pgid=,comm='], capture_output=True, text=True).stdout
    return [l.split(None, 2) for l in out.splitlines() if l.split()[1] == str(pgid)]
print('before:', [(m[0], m[2].split('/')[-1]) for m in members()])
for sig, wait in ((signal.SIGINT, 5), (signal.SIGTERM, 5), (signal.SIGKILL, 2)):
    try: os.killpg(pgid, sig)
    except ProcessLookupError: break
    t = time.time()
    while members() and time.time() - t < wait: time.sleep(0.2)
    print(f'after {signal.Signals(sig).name}:', [(m[0], m[2].split('/')[-1]) for m in members()])
    if not members(): break
