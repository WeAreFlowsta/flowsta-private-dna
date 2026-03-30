# Flowsta Private DNA

Private Holochain DNA for storing user data in an encrypted, zero-knowledge architecture.

## Architecture

This DNA implements **private entries** that sync via DHT gossip between authorized edge nodes only. Key features:

- ✅ **Client-side encryption**: All sensitive data encrypted with user's password before storing
- ✅ **Private entries**: Data stored on user's source chain, NOT public DHT
- ✅ **Zero-knowledge**: Flowsta staff cannot decrypt user data
- ✅ **Multi-node resilience**: Private entries sync between edge nodes via DHT
- ✅ **Membrane-protected**: Only authorized edge nodes can join the network

## Data Storage

### UserProfile (Private)
- **encrypted_email**: User's email encrypted with their password
- **display_name**: Public display name (also stored on public DHT)
- **created_at**: Account creation timestamp
- **updated_at**: Last profile update

### RecoveryPhrase (Private)
- **encrypted_mnemonic**: 24-word BIP39 phrase encrypted with password
- **verified**: Whether user has confirmed they saved it
- **created_at**: When recovery phrase was generated

### Session (Private)
- **encrypted_data**: Device info, IP address, user agent (encrypted)
- **session_id**: Session identifier for revocation
- **created_at**: Login timestamp
- **last_activity**: Last session activity

## Security Model

1. **Private Entries**: All entry types marked with `visibility = "private"`
   - Stored on user's source chain
   - NOT gossiped to public DHT
   - Only synced between authorized nodes on private DHT

2. **Encryption**: XSalsa20Poly1305 encryption for all sensitive fields
   - Password-derived key using Argon2
   - Client-side encryption (server never sees plaintext)
   - Each field has unique nonce

3. **Membrane Proof**: Only edge nodes with valid proof can join
   - Signed by Flowsta infrastructure keys
   - Prevents unauthorized DHT access
   - Community nodes can be added by signing their keys

4. **Agent Isolation**: Each user has separate source chain
   - Holochain conductor isolates data per agent
   - Keys stored in Lair keystore (password-encrypted)
   - No cross-user data access possible

## Building

```bash
# Install Rust and Holochain tools
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install holochain_cli

# Add wasm32 target
rustup target add wasm32-unknown-unknown

# Build DNA
./build.sh
```

Output files:
- `workdir/flowsta_private.dna` - DNA bundle
- `workdir/flowsta_private_happ.happ` - hApp bundle

## Testing Locally

```bash
# Start a local conductor
hc sandbox generate workdir --run=8888

# Install the hApp
hc sandbox call install-app workdir/flowsta_private_happ.happ

# Test zome functions
hc sandbox call zome private_data store_profile '{"encrypted_email": [...], "nonce": [...], "display_name": "Alice", ...}'
```

## Integration with Auth API

The Auth API (`/api`) will:
1. Install this hApp for each new user (same agent key as public hApp)
2. Call zome functions to store/retrieve private data
3. Never store private data in PostgreSQL (only `agent_pub_key` for lookup)

## Multi-Edge-Node Setup

All edge nodes must:
1. Use the same `network_seed` in `dna.yaml`
2. Have valid membrane proof to join
3. Run the same DNA version
4. Connect to each other via DHT discovery

Private entries will automatically sync between nodes via DHT gossip.

## Community Edge Nodes (Future)

Community members can run edge nodes by:
1. Installing the private DNA
2. Getting membrane proof signed by Flowsta
3. Joining the private DHT network

This increases resilience and decentralization while maintaining privacy.

## Production Checklist

- [ ] Implement proper membrane proof verification (see `validation.rs`)
- [ ] Deploy to staging edge nodes and test multi-node sync
- [ ] Security audit of encryption implementation
- [ ] Load test with multiple concurrent users
- [ ] Set up monitoring for DHT health
- [ ] Document operational procedures for edge node management

