# A1: cost of reading the detection markers for every folder in ~/Projects.
import os, json, time, sys
root = os.path.expanduser('~/Projects')
def detect(d):
    j = lambda f: os.path.join(d, f)
    ex = os.path.exists
    comp = pkg = {}
    if ex(j('composer.json')):
        try: c = json.load(open(j('composer.json'))); comp = {**c.get('require', {}), **c.get('require-dev', {})}
        except Exception: pass
    if ex(j('artisan')) and 'laravel/framework' in comp: return 'laravel'
    if ex(j('spark')): return 'ci4'
    if ex(j('system/CodeIgniter.php')) or ex(j('system/core/CodeIgniter.php')): return 'ci3'
    if ex(j('bin/console')) and 'symfony/framework-bundle' in comp: return 'symfony'
    if ex(j('wp-config.php')) or ex(j('wp-load.php')): return 'wordpress'
    if ex(j('composer.json')) or ex(j('index.php')) or ex(j('public/index.php')): return 'php'
    if ex(j('package.json')):
        try: p = json.load(open(j('package.json'))); deps = {**p.get('dependencies', {}), **p.get('devDependencies', {})}; scripts = p.get('scripts', {})
        except Exception: deps, scripts = {}, {}
        for k, v in (('next', 'next'), ('nuxt', 'nuxt')):
            if k in deps: return v
        if any(k in deps for k in ('express', 'fastify', 'koa', 'hono')): return 'node-server'
        if 'vite' in deps: return 'vite'
        if 'dev' in scripts or 'start' in scripts: return 'node'
    if ex(j('index.html')): return 'static'
    return None
dirs = [os.path.join(root, d) for d in sorted(os.listdir(root)) if not d.startswith('.') and os.path.isdir(os.path.join(root, d))]
for run in range(3):
    t = time.perf_counter(); res = {os.path.basename(d): detect(d) for d in dirs}; ms = (time.perf_counter() - t) * 1000
    print(f'run {run+1}: {len(dirs)} folders in {ms:.1f} ms')
print(json.dumps(res, indent=0))
