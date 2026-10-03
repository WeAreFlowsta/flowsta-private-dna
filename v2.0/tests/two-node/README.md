# Two-node check for the private network

Checks that the coordinator in this branch lets a person's devices share one set of records, and that large objects can be sent between them in pieces. Two ways to run it.

## Two machines on two networks (`two.mjs`)

Needs Node 18+ and Flowsta Vault installed (the script uses the Holochain binary the Vault carries; the Vault does not have to be running). The same commands on Linux, macOS and Windows.

    npm install
    node two.mjs host                 # first machine: writes 500 records, prints the line for the second
    node two.mjs join <seed> <base>   # second machine, on another network: paste the printed line

The second machine prints how long it took to hold every record, sends 50 MB to the first in pieces, then prints a line each time a heartbeat record arrives. Put it to sleep for a few minutes and wake it to see how fast it catches up. Ctrl+C stops either side. Options: `--records 500 --send-mb 50 --piece-kb 512 --window 4`. Conductor data goes to the temp folder (`fvsp-host` / `fvsp-join`); delete it to start over.

## One machine, step by step (`h.mjs`)

    HC=<holochain 0.6.1 binary> AUTH=<auth material> bash start.sh        # nodes a and b
    HC_CLIENT=<.../@holochain/client/lib/index.js> node h.mjs setup       # shipped bundle on both, one seed
    node h.mjs phase1     # shipped coordinator: a second device lists nothing
    node h.mjs swap       # UpdateCoordinators; the DNA hash must not change
    node h.mjs phase2     # list at the shared base, retire, supersede, markers
    node h.mjs bulk 500   # a realistic cell
    bash start.sh c && node h.mjs join    # a brand-new device catches up
    bash stop.sh a b c

`fixtures/` holds the shipped bundle (never rebuilt) and the coordinator built from this branch:

    RUSTFLAGS='--cfg getrandom_backend="custom"' cargo build --locked --release --target wasm32-unknown-unknown -p private_data_coordinator
