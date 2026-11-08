#!/bin/sh
# Runs under launchd (gui/<uid>), like the Devid daemon.
OUT="$1"
{
echo "== bare launchd PATH: $PATH"
for c in npm node cargo; do printf '%s: %s\n' "$c" "$(command -v $c || echo missing)"; done
for mode in "-l -c" "-l -i -c"; do
  t0=$(python3 -c 'import time;print(time.time())')
  /bin/zsh $mode 'env -0' > "$OUT.env" 2>"$OUT.err"
  t1=$(python3 -c 'import time;print(time.time())')
  P=$(tr '\0' '\n' < "$OUT.env" | sed -n 's/^PATH=//p')
  echo "== zsh $mode: $(python3 -c "print(round(($t1-$t0)*1000))") ms, stderr $(wc -c < "$OUT.err") bytes"
  for c in npm node cargo; do printf '  %s: %s\n' "$c" "$(PATH="$P" command -v $c || echo missing)"; done
done
} > "$OUT" 2>&1
