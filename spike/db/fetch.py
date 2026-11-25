#!/usr/bin/env python3
"""Download what the Phase 14 D1 spike needs on this computer, each archive checked against its
sha256 in the manifest (D-061).

  spike/db/fetch.py <work dir> [--only mysql-8.4,postgresql-18,...]

Every database series in devid-binaries' db-latest that has a build for this platform is unpacked
into <work>/<engine>-<series>/, mongosh into <work>/mongosh, Devid's PHP 8.5 from runtime-latest into
<work>/php and its mongodb extension into <work>/ext-mongodb. <work>/fetched.json lists them."""
import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys

BASE = "https://github.com/hadziqmtqn/devid-binaries/releases/download/"


def arch_key():
    system, machine = platform.system(), platform.machine()
    if system == "Darwin":
        return "arm64" if machine == "arm64" else "x86_64"
    if system == "Linux" and machine in ("x86_64", "AMD64"):
        return "linux-x86_64"
    sys.exit(f"no builds for {system} {machine}")


def get(url, dest):
    subprocess.run(["curl", "-fsSL", "--retry", "3", "-o", dest, url], check=True)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def fetch(entry, name, work):
    url, want = entry["url"], entry["sha256"]
    if not url.startswith(BASE):
        sys.exit(f"{name}: untrusted URL {url}")
    archive = os.path.join(work, "archives", os.path.basename(url))
    if not (os.path.exists(archive) and sha256(archive) == want):
        get(url, archive)
    got = sha256(archive)
    if got != want:
        sys.exit(f"{name}: sha256 {got}, manifest says {want}")
    dest = os.path.join(work, name)
    if not os.path.isdir(dest):
        tmp = dest + ".tmp"
        os.makedirs(tmp, exist_ok=True)
        subprocess.run(["tar", "-xzf", archive, "-C", tmp], check=True)
        os.rename(tmp, dest)
    return dest


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("work")
    ap.add_argument("--only", default="", help="comma-separated <engine>-<series> names")
    args = ap.parse_args()
    work, key = os.path.abspath(args.work), arch_key()
    only = {s for s in args.only.split(",") if s}
    os.makedirs(os.path.join(work, "archives"), exist_ok=True)
    manifests = {}
    for tag, name in (("db-latest", "db"), ("runtime-latest", "runtime")):
        path = os.path.join(work, f"{name}-manifest.json")
        get(f"{BASE}{tag}/manifest.json", path)
        with open(path) as f:
            manifests[name] = json.load(f)
    db, runtime = manifests["db"], manifests["runtime"]
    servers = []
    for engine in ("mysql", "postgresql", "mongodb"):
        for series, entry in db[engine].items():
            name = f"{engine}-{series}"
            build = entry["arch"].get(key)
            if not build or (only and name not in only):
                continue
            fetch(build, name, work)
            servers.append({"engine": engine, "series": series, "version": build["version"]})
            print(f"{name} {build['version']}", flush=True)
    if any(s["engine"] == "mongodb" for s in servers):
        fetch(db["mongosh"]["arch"][key], "mongosh", work)
    fetch(runtime["php"]["8.5"]["arch"][key], "php", work)
    fetch(runtime["extensions"]["8.5"]["mongodb"]["arch"][key], "ext-mongodb", work)
    with open(os.path.join(work, "fetched.json"), "w") as f:
        json.dump({"platform": key, "db_tag": db["tag"], "runtime_tag": runtime["tag"],
                   "servers": servers}, f, indent=1)


if __name__ == "__main__":
    main()
