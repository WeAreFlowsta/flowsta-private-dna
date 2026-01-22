# Security Policy

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please email security@flowsta.com with:

1. **Description** - Detailed explanation of the vulnerability
2. **Steps to Reproduce** - How to trigger the issue
3. **Impact** - Potential consequences (decryption, data leakage, etc.)
4. **Suggested Fix** - If you have one (optional)
5. **Your Contact Info** - For follow-up questions

### What to Expect

- **Initial Response**: Within 48 hours
- **Status Update**: Within 1 week
- **Fix Timeline**: Critical issues treated as P0 (immediate)
- **Credit**: We'll credit you in our security advisories (unless you prefer anonymity)

## Supported Versions

| Version | Status | Support |
|---------|--------|---------|
| v1.9 | ✅ Production | Actively maintained |
| v1.8 | ✅ Stable | Security fixes |
| v1.7 | ✅ Stable | Security fixes |
| v1.6 | ⚠️ Supported | Security fixes only |
| v1.5 | ⚠️ Supported | Security fixes only |
| < v1.5 | ❌ End of Life | No support |

## Zero-Knowledge Security Model

### Core Principle

The **Flowsta Private DNA** implements **true zero-knowledge encryption**:

- ✅ Encryption happens **client-side** in the user's browser
- ✅ Only **encrypted data** is stored on the DHT
- ✅ **No API server** can decrypt user data without the user's password
- ✅ **No DHT node** can read plaintext data
- ✅ **Flowsta staff** cannot access user data

### What This Means

**Opening this code does NOT compromise security** because:

1. **Encryption is client-side** - The DNA never sees plaintext
2. **Keys are user-derived** - From password via Scrypt KDF
3. **No master keys** - We cannot decrypt data even if we wanted to
4. **Open-source = verifiable** - You can audit the encryption

## Encryption Details

### Algorithm Stack

```
User Password
    ↓
Scrypt(password, salt, N=16384, r=8, p=1)
    ↓
256-bit key
    ↓
AES-256-GCM(plaintext, key, 12-byte nonce)
    ↓
{ciphertext, nonce, salt, auth_tag}
```

**Standards**:
- **Encryption**: AES-256-GCM (NIST-approved)
- **Key Derivation**: Scrypt (memory-hard, resistant to ASICs)
- **Nonce**: 12 bytes (96 bits) - standard for GCM
- **Salt**: 16+ bytes (128+ bits) - unique per encryption
- **Auth Tag**: 16 bytes (128 bits) - prevents tampering

### What's Encrypted

| Data | Encrypted | Stored Where |
|------|-----------|--------------|
| Email | ✅ Yes | Private DHT |
| Recovery Phrase | ✅ Yes | Private DHT |
| Username | ✅ Yes | Private DHT (optional) |
| Login Activity | ✅ Yes | Private DHT (optional) |
| OAuth Activity | ✅ Yes | Private DHT |
| Privacy Settings | ✅ Yes | Private DHT |
| Nonces/Salts | ❌ No (public) | Private DHT |
| Auth Tags | ❌ No (public) | Private DHT |

**Why nonces/salts/tags are public**:
- They're required for decryption (not secret)
- AES-GCM security doesn't depend on their secrecy
- Standard practice in cryptography

## Known Vulnerabilities (Historical)

### 1. Short Nonce Bug (v1.4) - FIXED in v1.5

**Issue**: v1.4 generated 8-byte nonces instead of 12-byte.

**Risk**: Reduced security margin (still secure but below standard).

**Fix**: v1.5 enforces 12-byte nonces.

**Status**: All users migrated to v1.5+.

### 2. Single-Level Update Chain (v1.2-v1.4) - FIXED in v1.5

**Issue**: Password changes only followed one level of update chain.

**Risk**: Second password change would lose data.

**Fix**: v1.5 recursively follows entire update chain.

**Status**: All users migrated to v1.5+.

## Threat Model

### What We Protect Against

✅ **Protected**:
- Malicious DHT nodes reading user data
- API server compromise (data still encrypted)
- Database dumps (only encrypted blobs)
- Network eavesdropping (TLS + encrypted payload)
- Rainbow table attacks (Scrypt + unique salts)

❌ **NOT Protected** (user responsibility):
- User loses password AND recovery phrase
- User computer compromised (keylogger, malware)
- User voluntarily shares password
- Weak passwords (though Scrypt helps)

### Attack Scenarios

| Attack | Mitigation |
|--------|-----------|
| **Brute force password** | Scrypt (expensive, memory-hard) |
| **Rainbow tables** | Unique salt per encryption |
| **Quantum computers** | AES-256 has 128-bit post-quantum security |
| **Network MITM** | TLS + authenticated encryption |
| **DHT node compromise** | Data pre-encrypted client-side |
| **API server compromise** | Zero-knowledge (can't decrypt) |

## Security Best Practices for Integrators

If you're integrating this DNA into your application:

### ✅ Do:
- Enforce strong password requirements
- Use PBKDF2/Scrypt for key derivation
- Always use 12-byte nonces for AES-GCM
- Validate salt length (16+ bytes)
- Never log plaintext data
- Use TLS for all network communication
- Follow the update chain recursively (critical!)

### ❌ Don't:
- Generate nonces shorter than 12 bytes
- Reuse nonces (cryptographic disaster)
- Store plaintext data "temporarily"
- Log encryption keys or passwords
- Trust v1.4 or earlier (known bugs)
- Only follow one level of update chain

## Vulnerability Examples

### Critical Severity
- Encryption bypass
- Key derivation weakness
- Nonce reuse
- Authentication tag bypass
- Update chain corruption

### High Severity
- Information leakage in error messages
- Timing attacks on encryption operations
- Improper key management

### Medium Severity
- Denial of service attacks
- Validation bypass (non-cryptographic)
- Memory leaks in encryption code

### Low Severity
- Documentation errors
- Non-exploitable edge cases
- Performance issues

## Security Audit History

- **October 2024**: Initial launch (v1.0)
- **November 2025**: v1.4 nonce bug discovered → Fixed in v1.5
- **November 2025**: Update chain bug discovered → Fixed in v1.5
- **January 2026**: Production deployment with 3-node redundancy

## Responsible Disclosure

We follow **coordinated disclosure**:

1. **Report received** → Acknowledge within 48 hours
2. **Assess severity** → P0 (critical) gets immediate attention
3. **Develop fix** → Tested on staging before production
4. **Deploy patch** → Production deployment (with user migration if needed)
5. **Public advisory** → After fix is deployed (credits researcher)

**Timeline**: 
- Critical issues: 72 hours to patch
- High issues: 2 weeks to patch
- Medium/Low: Next release cycle

## Bug Bounty

We currently don't have a formal bug bounty program, but we deeply appreciate security research and will:

- Credit you in our security advisories
- Consider financial compensation for critical vulnerabilities
- Fast-track your contributions
- Provide references for responsible disclosure

### High-Value Targets

- Encryption bypass vulnerabilities
- Key derivation weaknesses
- Update chain corruption bugs
- Data leakage in error handling

## Security FAQs

### Q: Why open-source a "Private" DNA?

**A**: "Private" refers to DHT visibility (`visibility = "private"`), not the code. Open-sourcing proves our zero-knowledge claims. Security through obscurity is not security.

### Q: Can Flowsta decrypt my data?

**A**: No. Mathematically impossible without your password. We only store encrypted blobs.

### Q: What if Flowsta gets hacked?

**A**: Attackers get encrypted data (useless without your password). Your data remains secure.

### Q: What about quantum computers?

**A**: AES-256 has 128-bit post-quantum security (considered safe). We'll upgrade if quantum threats materialize.

### Q: Can I audit the encryption myself?

**A**: Yes! That's why we're open-source. See `/v1.9/zomes/private_data/` for implementation.

## Contact

**Security Email**: security@flowsta.com  
**PGP Key**: (Coming soon)  
**Response Time**: 48 hours (24h for critical issues)

---

**Last Updated**: January 2026  
**Maintained by**: [Flowsta Security Team](https://flowsta.com)  
**Encryption**: AES-256-GCM + Scrypt (auditable code)
