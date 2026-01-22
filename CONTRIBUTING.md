# Contributing to Flowsta Private DNA

Thank you for your interest in contributing to Flowsta's zero-knowledge authentication system! This document provides guidelines for contributing to the Private DNA repository.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Migration Testing](#migration-testing)
- [Pull Request Process](#pull-request-process)
- [Versioning Strategy](#versioning-strategy)
- [Security](#security)

## Code of Conduct

### Our Standards

- **Be respectful** - Treat all contributors with respect
- **Be constructive** - Provide helpful feedback
- **Be collaborative** - Work together toward common goals
- **Focus on privacy** - User data security is paramount

### Unacceptable Behavior

- Harassment or discriminatory language
- Personal attacks or insults
- Publishing others' private information
- Compromising user privacy
- Other unprofessional conduct

## Getting Started

### Prerequisites

Before contributing, ensure you have:

- **Rust 1.75+** - `rustup` for Rust toolchain management
- **Holochain 0.6.0** - Install via `nix-shell https://holochain.love`
- **Holochain CLI** - `cargo install holochain_cli`
- **Git** - For version control
- **Understanding of cryptography** - AES-256-GCM, key derivation

### First-Time Setup

```bash
# 1. Fork the repository on GitHub

# 2. Clone your fork
git clone https://github.com/YOUR_USERNAME/flowsta-private-dna.git
cd flowsta-private-dna

# 3. Add upstream remote
git remote add upstream https://github.com/WeAreFlowsta/flowsta-private-dna.git

# 4. Build the latest version
cd v1.9
bash build.sh

# 5. Run tests
cargo test
```

## Development Setup

### Project Structure

```
flowsta-private-dna/
├── v1.0/ - v1.8/  # Historical versions
├── v1.9/          # CURRENT VERSION - Work here!
│   ├── dna.yaml       # DNA configuration
│   ├── happ.yaml      # hApp bundle definition
│   ├── build.sh       # Build script
│   └── zomes/
│       └── private_data/
│           ├── coordinator/   # Business logic
│           └── integrity/     # Entry type definitions
├── DNA_MIGRATION_GUIDE.md    # Migration documentation
└── README.md
```

### Building

```bash
cd v1.9

# Build DNA and hApp
bash build.sh

# Output: workdir/flowsta_private_v1_9_happ.happ
```

### Testing Locally

```bash
# Run Rust unit tests
cd v1.9/zomes/private_data/coordinator
cargo test

# Integration tests (requires running conductor)
# See DNA_MIGRATION_GUIDE.md for migration testing
```

## Making Changes

### Workflow

1. **Create a branch** from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** in the latest version directory (`v1.9/`)

3. **Test thoroughly** - Unit, integration, AND migration tests

4. **Commit with clear messages**:
   ```bash
   git commit -m "feat: Add privacy settings update validation"
   git commit -m "fix: Resolve update chain recursion bug"
   git commit -m "docs: Update encryption documentation"
   ```

5. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

6. **Open a Pull Request** on GitHub

### Commit Message Convention

We follow Conventional Commits:

- `feat:` - New feature
- `fix:` - Bug fix
- `security:` - Security improvement
- `docs:` - Documentation changes
- `test:` - Test additions or updates
- `refactor:` - Code refactoring
- `perf:` - Performance improvements

Examples:
```
feat: Add zero-knowledge analytics tracking
fix: Resolve recovery phrase update chain bug
security: Enforce 12-byte nonce minimum
docs: Update encryption algorithm documentation
```

## Testing

### Unit Tests

```bash
cd v1.9/zomes/private_data/coordinator
cargo test
```

### Critical Functions to Test

1. **Update chain following** - Test multiple password changes
2. **Encryption validation** - Test nonce/salt lengths
3. **Data retrieval** - Test after multiple updates
4. **Edge cases** - Empty fields, very long strings

### Test Coverage

We aim for:
- **90%+ code coverage** for coordinator zomes
- **100% coverage** for critical functions:
  - `get_user_profile` (must follow entire update chain)
  - `get_recovery_phrase` (must follow entire update chain)
  - `update_user_profile` (must handle repeated updates)
  - `update_recovery_phrase` (must handle repeated updates)

## Migration Testing

**CRITICAL**: Testing migrations is mandatory for any changes to entry types.

### Why Migration Testing Matters

Historical example: v1.4 → v1.5 migration failed because:
- v1.4 generated 8-byte nonces (bug)
- v1.5 validation rejected 8-byte nonces
- Users couldn't migrate → stuck on v1.4

**Lesson**: Always test migrations with accounts created on the old version.

### Migration Test Process

```bash
# 1. Build the old version (e.g., v1.8)
cd v1.8 && bash build.sh

# 2. Create test accounts on v1.8
# (Use your API or test script)

# 3. Change passwords on those accounts (tests update chain)

# 4. Build the new version (e.g., v1.9)
cd ../v1.9 && bash build.sh

# 5. Run migration for test accounts

# 6. Verify:
# - All data migrated correctly
# - Update chains preserved
# - No data loss
# - Password changes still work on v1.9
```

See [DNA_MIGRATION_GUIDE.md](DNA_MIGRATION_GUIDE.md) for detailed procedures.

## Pull Request Process

### Before Submitting

- ✅ Code builds successfully
- ✅ All unit tests pass
- ✅ Migration tests pass (if entry types changed)
- ✅ Code follows Rust style guidelines (`rustfmt`)
- ✅ Documentation updated (if needed)
- ✅ No linter warnings (`cargo clippy`)
- ✅ Security implications considered

### PR Template

When opening a PR, include:

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change (requires new DNA version)
- [ ] Documentation update
- [ ] Security improvement

## Testing
- [ ] Unit tests pass
- [ ] Migration tested (if applicable)
- [ ] Edge cases tested

## Security Implications
Any security considerations?

## Checklist
- [ ] Tests pass
- [ ] Documentation updated
- [ ] No new warnings
- [ ] Migration tested (if breaking change)
```

### Review Process

1. **Automated checks** - CI runs tests and linters
2. **Security review** - Critical for encryption code
3. **Code review** - Maintainers review your code
4. **Migration testing** - For breaking changes
5. **Approval** - Maintainers approve when ready
6. **Merge** - We merge into `main`

## Versioning Strategy

### When to Create a New Version

Create a new version (`v1.10/`, etc.) if:

- **Breaking changes** to entry types
- **Network seed change** required (forces new DHT)
- **Major new features** requiring migration
- **Encryption algorithm changes** (rare)

### Creating a New Version

```bash
# Copy the latest version
cp -r v1.9 v1.10

# Update configuration
cd v1.10
# Edit dna.yaml: Update network_seed to "flowsta-private-network-v1.10"
# Edit happ.yaml: Update version info
# Edit VERSION_README.md: Document changes

# Make your changes in zomes/

# Document migration path
# Update DNA_MIGRATION_GUIDE.md

# Test migration from v1.9 → v1.10 (CRITICAL!)
```

### Version Compatibility

| Version | Status | Notes |
|---------|--------|-------|
| v1.9 | ✅ Production | Current |
| v1.8 | ✅ Stable | Holochain 0.6 upgrade |
| v1.7 | ✅ Stable | Username support |
| v1.6 | ⚠️ Supported | Activity tracking |
| v1.5 | ⚠️ Supported | Update chain fix |
| < v1.5 | ❌ End of Life | Known bugs |

## Security

### Reporting Security Issues

**DO NOT** open public issues for security vulnerabilities.

Email: security@flowsta.com

See [SECURITY.md](SECURITY.md) for detailed reporting guidelines.

### Security Considerations

When contributing to this DNA:

- ✅ **Always use 12-byte nonces** for AES-256-GCM
- ✅ **Never log plaintext data** (even in development)
- ✅ **Validate encryption parameters** (nonce/salt lengths)
- ✅ **Follow update chains recursively** (critical!)
- ✅ **Test password change cycles** (2-3 changes)
- ❌ **Never weaken encryption** (no shortcuts)
- ❌ **Never store plaintext** (even temporarily)

### Cryptography Changes

If proposing changes to encryption:

1. **Justify the change** - Why is it needed?
2. **Cite standards** - NIST, RFC references
3. **Provide security analysis** - Attack resistance
4. **Test migration** - Old encrypted data must still decrypt
5. **Get security review** - Mandatory for crypto changes

## What We're Looking For

### High-Priority Contributions

- 🐛 **Bug fixes** - Especially in update chain logic
- 🔐 **Security improvements** - Encryption, validation
- 📚 **Documentation** - Especially encryption details
- ✅ **Test coverage** - Migration tests, edge cases
- ⚡ **Performance** - DHT operation optimization

### Ideas for Contributions

- Improved error messages (without leaking plaintext)
- Better validation of encrypted data
- Performance benchmarks
- Integration examples
- Migration automation tools
- Security audit tools

### Areas Needing Attention

- **Update chain testing** - More comprehensive tests
- **Migration automation** - Tools to simplify migrations
- **Performance profiling** - Identify bottlenecks
- **Error handling** - Better error messages

## Questions?

- **General questions**: Open a GitHub Discussion
- **Bug reports**: Open a GitHub Issue
- **Feature requests**: Open a GitHub Issue
- **Security issues**: Email security@flowsta.com
- **Migration help**: See DNA_MIGRATION_GUIDE.md

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.

---

Thank you for contributing to Flowsta's zero-knowledge authentication system! 🎉  
Your work helps protect user privacy globally.
