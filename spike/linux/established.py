"""Phase 12 L1, question 8: the Linux twin of ondemand.rs `established(pgid, port)`.

Whether a process of group `pgid` holds an established TCP connection on its local port `port`,
read from /proc without starting a process: the sockets in /proc/net/tcp{,6} in state 01
(ESTABLISHED) with that local port, matched by inode against the group's /proc/<pid>/fd links.

    python3 established.py <pgid> <port>      exit 0: yes, 1: no
"""

import os
import sys


def established_inodes(port):
    inodes = set()
    for table in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            with open(table) as f:
                next(f)
                for line in f:
                    fields = line.split()
                    local, state, inode = fields[1], fields[3], fields[9]
                    if state == "01" and int(local.rsplit(":", 1)[1], 16) == port:
                        inodes.add(inode)
        except FileNotFoundError:
            pass
    return inodes


def group_pids(pgid):
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            with open(f"/proc/{entry}/stat") as f:
                stat = f.read()
        except OSError:
            continue
        # The command name may hold spaces and parentheses: fields after the last ')' are fixed.
        fields = stat.rsplit(")", 1)[1].split()
        if int(fields[2]) == pgid:  # state, ppid, pgrp
            yield int(entry)


def established(pgid, port):
    inodes = established_inodes(port)
    if not inodes:
        return False
    for pid in group_pids(pgid):
        try:
            for fd in os.listdir(f"/proc/{pid}/fd"):
                link = os.readlink(f"/proc/{pid}/fd/{fd}")
                if link.startswith("socket:[") and link[8:-1] in inodes:
                    return True
        except OSError:
            continue
    return False


if __name__ == "__main__":
    sys.exit(0 if established(int(sys.argv[1]), int(sys.argv[2])) else 1)
