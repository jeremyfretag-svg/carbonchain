# Contributing to CarbonChain

Thank you for your interest in contributing to CarbonChain! This document provides guidelines and instructions for contributing to this project.

CarbonChain participates in the **Stellar Wave program on Drips** — a monthly, funded open-source contribution sprint. Contributors earn real rewards for merged pull requests. Read the [Stellar Wave section](#stellar-wave-contributions) before picking up an issue.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Environment Setup](#development-environment-setup)
- [Running Tests](#running-tests)
- [Code Style Guidelines](#code-style-guidelines)
- [Branch Naming Conventions](#branch-naming-conventions)
- [Pull Request Process](#pull-request-process)
- [Issue and PR Templates](#issue-and-pr-templates)
- [Stellar Wave Contributions](#stellar-wave-contributions)
- [Documentation](#documentation)

---

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct. Please be respectful and constructive in all interactions. Contributors who engage in harassment, bad-faith submissions, or manipulation of the Wave rewards system will be permanently removed from the program.

---

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally:

```bash
git clone https://github.com/legend-esc/carbonchain.git
cd carbonchain
```

3. Add upstream remote:

```bash
git remote add upstream https://github.com/legend-esc/carbonchain.git
```

4. Create a feature branch (see [Branch Naming Conventions](#branch-naming-conventions))

---

## Development Environment Setup

### Rust + Soroban Toolchain

CarbonChain smart contracts are built using Rust and the Soroban SDK for Stellar.

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Install Soroban CLI:

```bash
cargo install --locked soroban-cli
```

Add WASM target:

```bash
rustup target add wasm32-unknown-unknown
```

Verify installation:

```bash
rustc --version
soroban --version
```

---

### Node.js for API and Frontend

Both the NestJS API and Angular frontend require Node.js 18+.

Install Node.js via nvm (recommended):

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
nvm use 18
```

Install API dependencies:

```bash
cd api
npm install
```

Install frontend dependencies:

```bash
cd frontend
npm install
```

Install Angular CLI globally:

```bash
npm install -g @angular/cli
```

Verify installation:

```bash
node --version
npm --version
ng version
```

---

### Local Services

Start PostgreSQL via Docker:

```bash
docker compose up -d postgres
```

Run database migrations:

```bash
cd api && npm run migration:run
```

---

### Environment Variables

```bash
cp api/.env.example api/.env
cp frontend/src/environments/environment.example.ts \
   frontend/src/environments/environment.ts
```

Fill in your Stellar testnet keypair and other values. See `README.md` for the full variable reference.

---

### Deploy Contracts to Testnet

```bash
cd scripts
./deploy-testnet.sh
```

This funds a testnet account, compiles all four Soroban contracts, deploys them, and writes contract IDs to `scripts/contract-ids.testnet.json`.

---

### Environment Diagnostics

```bash
anchorkit doctor
```

The doctor command checks:

- ✅ Rust toolchain installation
- ✅ WASM target availability
- ✅ Wallet (Freighter) configuration
- ✅ Soroban RPC endpoint connectivity
- ✅ Config file validity
- ✅ Network connectivity

See `docs/guides/DOCTOR_COMMAND.md` for complete documentation.

---

## Running Tests

### Rust (Soroban contracts)

```bash
# Run all contract tests
cargo test

# Run with verbose output
cargo test --verbose

# Run a specific contract's tests
cargo test -p credit_registry

# Run tests with stdout
cargo test -- --nocapture

# Run cross-platform path tests
cargo test cross_platform
```

### NestJS API

```bash
cd api

# Unit tests
npm run test

# Integration / e2e tests
npm run test:e2e

# Coverage report
npm run test:cov
```

### Angular Frontend

```bash
cd frontend

# Run all tests (Jasmine + Karma)
ng test

# Run tests in watch mode
ng test --watch

# Run tests with coverage
ng test --code-coverage
```

### Configuration Validation

**Linux / macOS**

```bash
./validate_all.sh
./pre_deploy_validate.sh
```

**Windows**

```powershell
.\validate_all.ps1
.\pre_deploy_validate.ps1
```

---

## Code Style Guidelines

### Rust

Format your code with rustfmt:

```bash
cargo fmt
```

Lint with clippy:

```bash
cargo clippy -- -D warnings
```

- Add doc comments (`///`) for all public contract functions
- Use the `CarbonChainError` type for all error handling (see `docs/features/ERROR_CODES_REFERENCE.md`)
- Stable error codes 100–120 must not be changed — add new codes above 120 only

### NestJS (TypeScript)

Lint:

```bash
cd api
npm run lint
```

Type check:

```bash
cd api
npm run type-check
```

Format:

```bash
cd api
npm run format
```

- Use NestJS decorators consistently — no raw Express patterns
- All Stellar interactions go through `StellarService` — never call the SDK directly from controllers
- Inject dependencies via constructor injection, not property injection
- DTOs must use `class-validator` decorators for all request bodies

### Angular (TypeScript)

Lint:

```bash
cd frontend
ng lint
```

Type check:

```bash
cd frontend
npx tsc --noEmit
```

- Use standalone components (Angular 17+) — no NgModules for new components
- Use Angular Signals for local state — avoid direct `BehaviorSubject` patterns where signals suffice
- All HTTP calls go through `ApiService` — never use `HttpClient` directly in components
- All wallet interactions go through `StellarWalletService`

### General Guidelines

- Write clear, descriptive commit messages (see commit format below)
- Keep commits focused and atomic — one logical change per commit
- Add tests for all new functionality before submitting a PR
- Update relevant documentation in `docs/` alongside code changes
- Follow existing patterns in the codebase before introducing new ones

---

## Branch Naming Conventions

Use descriptive branch names with the following prefixes:

| Prefix | Use for |
|---|---|
| `feature/` | New features |
| `fix/` | Bug fixes |
| `docs/` | Documentation updates |
| `refactor/` | Code refactoring without behavior change |
| `test/` | Test additions or modifications |
| `chore/` | Maintenance, dependency updates |
| `contract/` | Soroban contract changes specifically |

**Examples:**

```
feature/retirement-certificate-pdf
fix/verifier-multisig-edge-case
contract/mrv-oracle-threshold-config
docs/update-api-spec
test/credit-registry-integration
```

---

## Pull Request Process

1. Update your branch with the latest upstream changes:

```bash
git fetch upstream
git rebase upstream/main
```

2. Ensure all tests pass:

```bash
# Contracts
cargo test

# API
cd api && npm run test && npm run test:e2e

# Frontend
cd frontend && ng test --watch=false
```

3. Run all linters:

```bash
# Contracts
cargo fmt
cargo clippy -- -D warnings

# API
cd api && npm run lint && npm run format

# Frontend
cd frontend && ng lint
```

4. Commit your changes using conventional commit format:

```bash
git add .
git commit -m "feat(retirement): add PDF certificate generation"
```

Commit types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `contract`

5. Push to your fork:

```bash
git push origin feature/your-feature-name
```

6. Open a Pull Request on GitHub against `main` and fill out the PR template.

### PR Review Process

- All PRs require at least one maintainer approval before merge
- Address review feedback within 48 hours (especially during a Wave window)
- Keep PRs focused — one feature or fix per PR
- Update `docs/` and `CHANGELOG.md` if your change affects public behavior
- All CI checks (tests, lint, contract build) must pass before merge

---

## Issue and PR Templates

### Issue Template

When creating an issue, please include:

- **Description:** Clear description of the bug or feature request
- **Steps to Reproduce:** For bugs, numbered steps to reproduce
- **Expected Behavior:** What should happen
- **Actual Behavior:** What actually happens
- **Environment:** OS, Rust version, Node version, browser (if frontend)
- **Contract / Layer:** Which layer is affected (contracts / api / frontend)
- **Screenshots:** If applicable

### PR Template

When creating a PR, please include:

- **Description:** What does this PR do and why?
- **Related Issues:** Link to related issues (e.g., `Closes #42`)
- **Type of Change:** Bug fix · Feature · Contract change · Documentation · Refactor
- **Layer(s) Affected:** Contracts · API · Frontend · Shared · Scripts
- **Testing:** How was this tested? Testnet tx hashes if applicable

**Checklist:**

- [ ] Tests added or updated
- [ ] Documentation updated
- [ ] Rust: `cargo fmt` and `cargo clippy` pass
- [ ] API: `npm run lint` and `npm run test` pass
- [ ] Frontend: `ng lint` and `ng test` pass
- [ ] No secrets or private keys committed
- [ ] `CHANGELOG.md` updated if this is a user-facing change

---

## Stellar Wave Contributions

CarbonChain participates in the **Stellar Wave program on Drips**. During each Wave (a 7-day sprint), contributors solve labeled issues and earn Points that translate into real rewards funded by the Stellar Development Foundation.

### How to participate

1. Browse issues labelled `stellar-wave` on this repo
2. Sign in at [drips.network/wave/stellar](https://drips.network/wave/stellar) with your GitHub account
3. Complete KYC in Settings → Identity and Payments (required to withdraw rewards)
4. Apply to work on an issue through the Drips Wave app
5. Wait for maintainer assignment — usually within 24 hours
6. Submit a PR within the 7-day Wave window
7. Earn Points proportional to issue complexity → earn a share of the reward pool

### Issue complexity levels

| Level | Points | Typical scope |
|---|---|---|
| Trivial | 100 | Typo fix, small config, minor copy change |
| Medium | 150 | Standard feature, involved bug fix |
| High | 200 | Complex feature, refactor, new integration |

### Rules and expectations

- You must understand and be able to explain your submission — contributions generated entirely by LLMs without review are **prohibited** under Drips Wave terms and will be rejected
- Apply only to issues you have reviewed and are confident you can complete within the Wave window
- You may hold up to 15 pending applications and be assigned at most 7 issues per organization per Wave
- Compliments (bonus rewards) are given by maintainers for exceptional work — do not expect them for every contribution
- Low-quality, copy-paste, or bad-faith submissions will result in disqualification from the program

See [Drips Wave Terms](https://docs.drips.network/wave/terms-and-rules/) for the full rules.

---

## Documentation

### Existing Documentation

| File | Purpose |
|---|---|
| `README.md` | Main project documentation and quick start |
| `ARCHITECTURE.md` | Full system architecture and design decisions |
| `QUICK_START.md` | Quick reference with examples |
| `CHANGELOG.md` | Version history |
| `docs/features/ERROR_CODES_REFERENCE.md` | Stable API error codes |
| `docs/features/SEP10_AUTH.md` | SEP-10 authentication |
| `docs/features/TRANSACTION_STATE_TRACKER.md` | Credit lifecycle state machine |
| `docs/guides/DOCTOR_COMMAND.md` | CLI diagnostics |
| `docs/guides/ERROR_IMPLEMENTATION_GUIDE.md` | Error handling guide |

See `docs/README.md` for the complete documentation index.

### Writing Documentation

- Use clear, concise language — write for a developer seeing this for the first time
- Include working code examples for every feature documented
- Keep documentation in sync with code — a PR that changes behavior must update the relevant doc
- Use Markdown formatting consistently with the existing docs style
- For contract functions, document parameters, return values, error codes, and side effects

---

## Questions or Issues?

If you have questions or encounter issues:

- Check the `docs/` documentation files
- Review the Swagger UI at `http://localhost:3000/api/docs`
- Examine the test cases in `contracts/*/src/lib.rs`
- Search existing GitHub issues
- Open a new issue if your question is not already answered

---

Thank you for contributing to CarbonChain!

> Repository: [https://github.com/legend-esc/carbonchain.git](github-legend-esc:legend-esc/carbonchain.git)
