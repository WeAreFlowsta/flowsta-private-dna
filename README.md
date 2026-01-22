# Flowsta Private DNA

**Zero-knowledge encrypted user data storage for Flowsta Auth**

[![Status](https://img.shields.io/badge/status-production-brightgreen.svg)](https://flowsta.com)
[![Holochain](https://img.shields.io/badge/holochain-0.6.0-blue.svg)](https://holochain.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![DNA Version](https://img.shields.io/badge/DNA-v1.9-orange.svg)](#version-history)

> **🎉 Production Status**: This DNA is currently running in production, powering [Flowsta Auth](https://flowsta.com) with true zero-knowledge encryption.

This repository contains all versions of the Flowsta Private DNA - a Holochain DNA that stores encrypted user profiles, recovery phrases, and email permissions with complete client-side encryption.

---

## 🔒 What is This?

The **Flowsta Private DNA** is a critical component of our zero-knowledge authentication system. It stores sensitive user data in **encrypted form only** - the API and DHT nodes never see plaintext data.

### What's Stored (All Client-Side Encrypted):

- ✅ **User Profiles** - Encrypted email addresses (AES-256-GCM)
- ✅ **Recovery Phrases** - Encrypted BIP39 mnemonics for account recovery
- ✅ **Email Permissions** - User-granted permissions for services (e.g., billing notifications)
- ✅ **Activity Tracking** - Login history, OAuth app usage, dashboard visits (user-owned)
- ✅ **Privacy Settings** - User controls for IP tracking, retention policies
- ✅ **Usernames** - Optional, globally unique, encrypted usernames
- ✅ **Analytics IDs** - Zero-knowledge analytics (impossible to link to user DID)

**Zero-Knowledge Architecture**: All sensitive data is encrypted in the browser **before** being stored on the DHT. Even Flowsta staff cannot access plaintext user data without the user's password.

---

## 📁 Repository Structure

```
flowsta-private-dna/
├── README.md                    # This file
├── v1.0/                        # Original version (Oct 2024)
├── v1.1/                        # Added EmailPermission entry type (Oct 2024)
├── v1.3/                        # Fixed get_user_profile update chain (Oct 2024)
├── v1.4/                        # Fixed get_recovery_phrase + data validation (Oct 2024)
├── v1.5/                        # Recursive update chain fix (Nov 2025)
├── v1.6/ - v1.8/                # Additional features and improvements
└── v1.9/                        # ✅ CURRENT - Zero-knowledge analytics (Jan 2026)
```

**Note**: v1.2 was skipped in our versioning for historical reasons.

---

## 🏗️ Version History

| Version | Date | Type | Changes | Status |
|---------|------|------|---------|--------|
| **v1.9** | Jan 2026 | Feature | Zero-knowledge analytics (AppAnalyticsId) | ✅ **Production** |
| v1.8 | Nov 2025 | Upgrade | Holochain 0.6 upgrade (HDK 0.6.0, 23 breaking changes) | ✅ Stable |
| v1.7 | Nov 2025 | Feature | Username support + dashboard activity tracking | ✅ Stable |
| v1.6 | Nov 2025 | Feature | Login/OAuth/Dashboard activity + Privacy settings | ✅ Stable |
| v1.5 | Nov 2025 | **Critical Fix** | Recursive update chain following (password change fix) | ✅ Stable |
| v1.4 | Oct 2024 | Bug Fix | Fixed `get_recovery_phrase`, added `update_recovery_phrase` | ⚠️ Has nonce bug |
| v1.3 | Oct 2024 | Bug Fix | Fixed `get_user_profile` to follow update chain | ✅ Stable |
| v1.1 | Oct 2024 | Feature | Added `EmailPermission` entry type | ✅ Stable |
| v1.0 | Oct 2024 | Initial | Base implementation with UserProfile and RecoveryPhrase | ✅ Stable |

### Critical Lessons Learned

- **v1.2-v1.4**: All had the "single `.last()`" bug where update chains were only followed one level deep
- **v1.4**: Had an encryption bug generating 8-byte nonces instead of 12-byte, making migrations to v1.5 fail
- **v1.5**: Fixed with recursive loop to traverse the ENTIRE update chain
- **Key Insight**: ALWAYS test migrations with fresh accounts on the old version before production!

---

## 🚀 Quick Start

### Building a DNA

```bash
# Navigate to any version
cd v1.5

# Build the DNA and hApp
bash build.sh

# Output: workdir/flowsta_private_v1_5_happ.happ
```

### Installing on Conductor

```bash
# Install via Holochain Admin API
hc app install ./v1.5/workdir/flowsta_private_v1_5_happ.happ
```

---

## 🧬 DNA Entry Types

### 1. UserProfile

Stores encrypted user email and metadata.

```rust
pub struct UserProfile {
    pub encrypted_email: String,  // AES-256-GCM encrypted email
    pub nonce: String,             // 12-byte nonce (24 hex chars)
    pub salt: String,              // Scrypt salt (32+ hex chars)
    pub tag: String,               // Auth tag for GCM
    pub display_name: Option<String>,
    pub created_at: i64,
}
```

### 2. RecoveryPhrase

Stores encrypted BIP39 mnemonic for account recovery.

```rust
pub struct RecoveryPhrase {
    pub encrypted_mnemonic: String,  // AES-256-GCM encrypted 12-word phrase
    pub nonce: String,                // 12-byte nonce
    pub salt: String,                 // Scrypt salt
    pub tag: String,                  // Auth tag
    pub verified: bool,               // User has confirmed they saved it
    pub created_at: i64,
}
```

### 3. EmailPermission (v1.1+)

Stores user-granted email access permissions for specific services.

```rust
pub struct EmailPermission {
    pub service_name: String,         // e.g., "billing"
    pub purpose: String,              // Human-readable purpose
    pub granted: bool,                // Permission status
    pub granted_at: i64,
    pub revoked_at: Option<i64>,
}
```

---

## 🔐 Encryption Details

**Algorithm**: AES-256-GCM  
**Key Derivation**: Scrypt (N=16384, r=8, p=1)  
**Nonce Size**: 12 bytes (96 bits) - **CRITICAL!**  
**Salt Size**: 16+ bytes (128+ bits)  
**Auth Tag**: 16 bytes (128 bits)

### Encryption Flow

```
User Password
    ↓
Scrypt(password, salt) → 256-bit key
    ↓
AES-256-GCM(plaintext, key, nonce) → ciphertext + auth tag
    ↓
Store: { ciphertext, nonce, salt, tag }
```

### Decryption Flow (Client-Side Only!)

```
Retrieve: { ciphertext, nonce, salt, tag }
    ↓
Scrypt(password, salt) → 256-bit key
    ↓
AES-256-GCM-Decrypt(ciphertext, key, nonce, tag) → plaintext
```

---

## 🔄 Migration Process

When migrating users between DNA versions:

1. **Export** data from old DNA version
2. **Validate** exported data (encryption parameters, field lengths)
3. **Install** new DNA version
4. **Import** data to new DNA
5. **Verify** imported data (field comparison, update chain test)
6. **Update** database to new version

**Note**: DNA version migrations require careful planning and thorough testing to ensure data integrity.

---

## 🛠️ Development

### Prerequisites

- Rust 1.70+
- Holochain 0.2.x
- `hc` CLI tool

### Building Locally

```bash
# Install Holochain
nix-shell https://holochain.love

# Build any version
cd v1.5 && bash build.sh
```

### Testing

```bash
# Run Rust tests
cd v1.5/zomes/private_data/coordinator
cargo test

# Integration tests
hc sandbox create
hc sandbox run
# Then test via API calls
```

---

## 📚 Additional Documentation

- **[v1.0/README.md](./v1.0/README.md)** - Original version notes
- **[v1.1/README.md](./v1.1/README.md)** - EmailPermission feature
- **[v1.3/README.md](./v1.3/README.md)** - Update chain fix v1
- **[v1.4/README.md](./v1.4/README.md)** - Recovery phrase fix (⚠️ nonce bug)
- **[v1.5/README.md](./v1.5/README.md)** - Recursive update chain fix

---

## 🚨 Critical Bugs to Avoid

### 1. Not Following Update Chains Recursively (v1.2-v1.4 bug)

**Symptom**: First password change works, second fails  
**Cause**: Only following ONE level of the update chain  
**Fix**: Use a loop to traverse the ENTIRE chain

```rust
// ❌ BAD: Only follows last update once
if let Some(updates) = record_details.updates.last() {
    return get(updates.action_address().clone(), GetOptions::default());
}

// ✅ GOOD: Loops until no more updates
loop {
    let details = get_details(current_hash.clone(), GetOptions::default())?;
    match details {
        Details::Record(record_details) => {
            if let Some(latest_update) = record_details.updates.last() {
                current_hash = latest_update.action_address().clone(); // Continue
            } else {
                return Ok(Some(record_details.record)); // Latest!
            }
        }
        _ => return Err(wasm_error!("Expected Record details")),
    }
}
```

### 2. Short Nonces (v1.4 bug)

**Symptom**: Data validation fails with "nonce too short"  
**Cause**: Generating 8-byte nonces instead of 12-byte  
**Fix**: Always use 12-byte (96-bit) nonces for AES-256-GCM

```javascript
// ❌ BAD: 8-byte nonce (insecure!)
const nonce = crypto.randomBytes(8);

// ✅ GOOD: 12-byte nonce (standard for GCM)
const nonce = crypto.randomBytes(12);
```

### 3. Missing `update_*` Functions

**Symptom**: Can't change passwords, data loss  
**Cause**: No `update_user_profile` or `update_recovery_phrase` functions  
**Fix**: Always implement update functions for mutable entries

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

### Quick Start for Contributors

1. **Fork the repository**
2. **Create a feature branch** (`git checkout -b feature/amazing-improvement`)
3. **Make your changes** in the latest version directory (currently v1.9)
4. **Test thoroughly** - Both unit tests and migration tests
5. **Submit a pull request**

### Creating a New DNA Version

For breaking changes or new features that require a new network seed:

```bash
# Copy the latest version
cp -r v1.9 v1.10

# Update DNA configuration
cd v1.10
# Edit dna.yaml: Update network_seed to "flowsta-private-network-v1.10"
# Edit happ.yaml: Update version info

# Make your changes in zomes/

# Test migration from v1.9 → v1.10
# Document the migration process
```

**CRITICAL**: Always test migrations from the previous version with fresh accounts before merging!

### Code of Conduct

- Be respectful and constructive
- Security vulnerabilities → email security@flowsta.com (not public issues)
- Focus on user privacy and zero-knowledge principles
- Test thoroughly - user data is at stake

### What We're Looking For

- 🔐 Security improvements
- ⚡ Performance optimizations
- 📚 Documentation enhancements
- 🐛 Bug fixes (especially in update chain logic)
- ✅ Additional test coverage

---

## 🚨 Security

### Reporting Vulnerabilities

If you discover a security vulnerability, please email **security@flowsta.com** with:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

**Please do not** open a public GitHub issue for security vulnerabilities.

We aim to respond within 48 hours and will credit researchers in our security advisories.

### Why Open-Sourcing Improves Security

This DNA is open-source because:
- ✅ **Transparency proves zero-knowledge claims** - You can verify encryption happens client-side
- ✅ **Independent security audits** - External researchers can review the code
- ✅ **"Don't trust, verify" principle** - Unlike closed-source competitors
- ✅ **Community contributions** - Security improvements from the Holochain community

**Critical**: The DNA code cannot decrypt user data without their password. Opening the code does not compromise security.

### Security Audit History

- **November 2025**: v1.5 recursive update chain fix (password change bug)
- **January 2026**: Production deployment with multi-node DHT

We welcome independent security audits of this code.

---

## 📝 License

Copyright © 2024-2026 Flowsta

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

**Why Apache 2.0?**
- **Patent protection** - Explicit patent grant prevents contributors from later suing you
- **Enterprise-friendly** - Companies like Google, Facebook require patent clarity
- **Prevents patent trolls** - Critical for authentication/security software
- **Still permissive** - Allows commercial use, modification, distribution
- **Consistent with Holochain** - Same license as the underlying framework

---

## 🙏 Acknowledgments

Built with [Holochain](https://holochain.org) - A framework for distributed applications.

Special thanks to the Holochain community for guidance on:
- Zero-knowledge data storage patterns
- DHT gossip protocols
- Update chain management
- Security best practices

---

## 🔗 Related Projects

- **[Flowsta Identity DNA](https://github.com/WeAreFlowsta/flowsta-identity-dna)** - Companion DNA for public identity data
- **Flowsta Auth API** - Backend service (integration layer)
- **Flowsta Website** - User-facing application
- **[Flowsta Developer Portal](https://dev.flowsta.com)** - Integration guides for developers

---

## ❓ FAQ

### Why is this called "Private" DNA if it's open-source?

"Private" refers to the Holochain visibility setting (`visibility = "private"`), not the code itself. This means:
- ✅ The **code is open-source** (you're reading it!)
- ✅ The **data visibility is private** (encrypted, only accessible to the user)
- ✅ DHT nodes cannot read the plaintext data without the user's password

### Can Flowsta staff decrypt my data?

**No.** The encryption happens **client-side** in your browser before data reaches our servers. We only store encrypted blobs. Without your password, the data is unreadable.

### What if I lose my password?

You can recover your account using your **24-word recovery phrase** (also encrypted and stored in this DNA). Save it securely when you first create your account!

### How is this different from "Login with Google"?

| Feature | Flowsta (This DNA) | Google/Auth0 |
|---------|-------------------|--------------|
| **Your data** | Encrypted, you control | Google can read everything |
| **Open-source** | ✅ Yes (verify our claims) | ❌ No (trust them blindly) |
| **Censorship-resistant** | ✅ Distributed DHT | ❌ Google's servers |
| **Zero-knowledge** | ✅ Mathematically impossible to decrypt | ❌ Google has keys |

### Can other DHT nodes spy on my data?

No. The data is encrypted **before** being stored on the DHT. Other nodes only see encrypted blobs + public metadata (nonces, salts, auth tags). Without your password, it's computationally infeasible to decrypt.

---

**Status**: ✅ Production (v1.9)  
**Last Updated**: January 2026  
**Maintained by**: [Flowsta Team](https://flowsta.com)

