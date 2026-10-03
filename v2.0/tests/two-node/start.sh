#!/bin/bash
# Start two conductors (a, b) that mirror the Vault's conductor config, on the staging rendezvous.
set -e
S=/tmp/fvsp-run
# HC = the holochain 0.6.1 binary the Vault bundles; AUTH = the rendezvous auth material (never committed).
: "${HC:?set HC to the holochain binary}"; : "${AUTH:?set AUTH to the bootstrap auth material}"
BOOT="${BOOT:-bootstrap-staging.flowsta.com}"
for n in ${@:-a b}; do
  case "$n" in a) port=46001;; b) port=46002;; c) port=46003;; esac
  mkdir -p "$S/$n/data" "$S/$n/ks"
  cat > "$S/$n/conductor-config.yaml" <<YAML
data_root_path: '$S/$n/data'
keystore:
  type: lair_server_in_proc
  lair_root: '$S/$n/ks'
admin_interfaces:
- driver:
    type: websocket
    port: $port
    allowed_origins: '*'
network:
  bootstrap_url: https://$BOOT
  signal_url: wss://$BOOT
  relay_url: https://$BOOT./
  base64_auth_material_bootstrap: "$AUTH"
  base64_auth_material_relay: "$AUTH"
  request_timeout_s: 240
YAML
  if [ -f "$S/$n/pid" ] && kill -0 "$(cat "$S/$n/pid")" 2>/dev/null; then echo "$n already running"; continue; fi
  ( echo "spike-passphrase" | setsid nohup "$HC" --piped -c "$S/$n/conductor-config.yaml" >> "$S/$n/holochain.log" 2>&1 & echo $! > "$S/$n/pid" )
  echo "started $n on admin port $port"
done
