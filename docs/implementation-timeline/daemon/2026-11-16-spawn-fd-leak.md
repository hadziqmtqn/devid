# Fix: a service could stay "starting" for days because a sibling held its exec pipe

> Date: 2026-11-16 · Branch: `fix/spawn-fd-leak`

## Problem

On 2026-11-14 the owner's daemon reported PHP 8.4 as `starting`, with no pid, while php-fpm 8.4 (pid 63899) had been serving for 1 day 18 hours. PHP 8.5 was `running`. `daemon.log` showed no crash of php-fpm 8.4. The daemon had restarted at 19:48 on 2026-11-12, and php-fpm 8.4 had started one second later. On that start the daemon also started php-fpm 8.5 and nginx, and four site commands again. The Logs page showed the PHP 8.4 log as stopped.

## Findings

- The supervisor reports `starting` while a slot's pid is 0. The watch thread stores the pid only after `Command::spawn()` returns, and nothing in between could block for days. php-fpm had started, so `spawn()` itself never returned.
- With `pre_exec`, std forks and then waits on a pipe until the child's exec closes it. On macOS std creates that pipe with `pipe()` and marks it close-on-exec in a second step, since macOS has no `pipe2`. A child forked by another thread in that moment keeps the write end across its own exec. The waiting `spawn()` then returns only when that child exits. Site commands, php-fpm and nginx run for days.
- Reproduced on the owner's Mac with a standalone program. Six threads at a time spawned `sleep 3` with `pre_exec`, 300 rounds. 27 of 1,800 `spawn()` calls blocked for exactly 3 s. With the fix, none blocked, and the slowest took 124 ms.
- The same leak can hold other fds: the stdout pipe of a short probe (`ps`, `lsof`), or a connection to `devid.sock`, which would leave a client waiting for the end of a reply.
- After the reboot on 2026-11-15 nothing was held. `lsof` showed no pipe or socket of the daemon in its children, so the race is rare.

## Fix

- `supervisor::keep_no_fds(cmd)` adds a `pre_exec` step that marks every fd above stderr close-on-exec. At exec the child keeps only stdin, stdout and stderr.
- The supervisor (php-fpm and the web server) and site commands, which include Share's cloudflared, both use it. Short-lived children (`ps`, `lsof`, probes) do not, because they exit within seconds.
- The sweep stops at `getdtablesize()`, capped at 4096. launchd gives the daemon 256 fds, and sweeping 3–256 takes 0.17 ms. A shell may allow 61,440, which would cost about 45 ms per child without the cap.
- Test `a_service_keeps_none_of_the_daemons_fds`: a pipe without close-on-exec, a service started through the supervisor, and the read end must see EOF once the test closes its write end. It fails without the fix (the read waits until the 5 s timeout) and passes with it.
- Not covered by a test: the site command path calls the same helper, but a unit test would have to run the user's login shell.
