# A1 spike: run a dev command the way the daemon would (login-shell env + nvm bin, own session,
# stdin /dev/null, output to a log file), then stop the whole process group.
import os, sys, time, signal, socket, subprocess, json
cwd, cmd, port, log = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
home = os.path.expanduser('~')
base = {'HOME': home, 'USER': os.environ['USER'], 'SHELL': '/bin/zsh', 'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'en_US.UTF-8'}
t0 = time.time()
raw = subprocess.run(['/bin/zsh', '-l', '-c', 'env -0'], env=base, capture_output=True, timeout=20).stdout
env = dict(kv.split('=', 1) for kv in raw.decode().split('\0') if '=' in kv)
alias = open(f'{home}/.nvm/alias/default').read().strip()
vers = sorted((v for v in os.listdir(f'{home}/.nvm/versions/node') if v.lstrip('v').startswith(alias.lstrip('v'))), key=lambda v: [int(x) for x in v.lstrip('v').split('.')])
env['PATH'] = f'{home}/.nvm/versions/node/{vers[-1]}/bin:' + env['PATH']
cap_ms = round((time.time() - t0) * 1000)
out = open(log, 'wb')
p = subprocess.Popen(['/bin/zsh', '-c', cmd], cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT, start_new_session=True)
def up():
    try: socket.create_connection(('127.0.0.1', port), 0.3).close(); return True
    except OSError: return False
t1 = time.time()
while not up() and time.time() - t1 < 120 and p.poll() is None: time.sleep(0.3)
print(json.dumps({'env_capture_ms': cap_ms, 'node': vers[-1], 'pid': p.pid, 'pgid': os.getpgid(p.pid), 'ready_s': round(time.time() - t1, 1), 'listening': up()}))
