#!/bin/bash
S=/tmp/fvsp-run
for n in ${@:-a b}; do
  if [ -f "$S/$n/pid" ]; then p="$(cat "$S/$n/pid")"; kill "$p" 2>/dev/null && echo "stopped $n ($p)"; rm -f "$S/$n/pid"; fi
done
