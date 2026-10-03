# Two-node check for the private network

Runs two (or three) conductors with different agent keys on one network seed and checks that the coordinator in this branch lets a person's devices share one set of records.

Build only the coordinator (the shipped bundle in `workdir/` is never rebuilt):

    RUSTFLAGS='--cfg getrandom_backend="custom"' cargo build --locked --release --target wasm32-unknown-unknown -p private_data_coordinator

Run (conductor data lives in `/tmp/fvsp-run`; the path must be short - the key store's socket path has a length limit):

    HC=<holochain 0.6.1 binary> AUTH=<auth material> bash start.sh        # nodes a and b
    HC_CLIENT=<.../@holochain/client/lib/index.js> node h.mjs setup       # shipped bundle on both, one seed
    node h.mjs phase1     # shipped coordinator: a second device lists nothing
    node h.mjs swap       # UpdateCoordinators; the DNA hash must not change
    node h.mjs phase2     # list at the shared base, retire, supersede, markers
    node h.mjs bulk 500   # a realistic cell
    bash start.sh c && node h.mjs join    # a brand-new device catches up
    bash stop.sh a b c
