# Linera REST API Server for Termux

A Rust REST API server that wraps Linera service, enabling wallet management and GraphQL proxying on Android/Termux.

---

## User Review Required

> [!TIP]
> **GitHub Actions Compilation**: We'll use GitHub Actions with ARM64 cross-compilation to build binaries. Then download via `gh` CLI and copy to Termux via SCP. This is much faster than compiling on the device!

> [!WARNING]  
> **Memory Storage Only**: To simplify compilation, we'll use memory storage instead of RocksDB. This means **chain data is lost on restart**. For persistence, we'd need to compile with RocksDB feature (more complex).

> [!CAUTION]
> **Experimental**: Running blockchain infrastructure on Android/Termux is unconventional. This is suitable for development/testing, not production validators.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Android/Termux                           │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              linera-rest-server (Rust)              │   │
│  │                    Port: 3000                       │   │
│  │                                                     │   │
│  │  REST Endpoints:                                    │   │
│  │  POST /service/start   - Start linera service      │   │
│  │  POST /service/stop    - Stop linera service       │   │
│  │  GET  /service/status  - Check if running          │   │
│  │  POST /wallet/init     - Initialize wallet         │   │
│  │  POST /owner/add       - Add owner by pubkey       │   │
│  │  POST /graphql         - Proxy to linera GraphQL   │   │
│  │  GET  /chain/:id       - Get chain info            │   │
│  └───────────────────────────┬─────────────────────────┘   │
│                              │                             │
│                              │ Spawns/Manages              │
│                              ▼                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │           linera service (GraphQL)                  │   │
│  │                    Port: 8080                       │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Proposed Changes

### Phase 1: GitHub Actions ARM Cross-Compilation

We'll create a GitHub Actions workflow that:
1. Cross-compiles `linera-service` for `aarch64-unknown-linux-gnu`
2. Creates a GitHub Release with the binary as an artifact
3. You download it via `gh release download` and SCP to Termux

#### [NEW] [.github/workflows/build-arm.yml](file:///home/uratmangun/CascadeProjects/linera-termux/.github/workflows/build-arm.yml)

```yaml
name: Build Linera ARM64

on:
  workflow_dispatch:  # Manual trigger

jobs:
  build-arm64:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout linera-protocol
        uses: actions/checkout@v4
        with:
          repository: linera-io/linera-protocol
          
      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          targets: aarch64-unknown-linux-gnu
          
      - name: Install cross-compilation tools
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
          sudo apt-get install -y protobuf-compiler
          
      - name: Build for ARM64
        env:
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc
          CC_aarch64_unknown_linux_gnu: aarch64-linux-gnu-gcc
          CXX_aarch64_unknown_linux_gnu: aarch64-linux-gnu-g++
        run: |
          cargo build -p linera-service --release \
            --target aarch64-unknown-linux-gnu \
            --no-default-features \
            --features wasmer
            
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: linera-arm64
          path: target/aarch64-unknown-linux-gnu/release/linera
```

#### Download and Deploy Script
```bash
# On your Linux machine:
# 1. Trigger the workflow
gh workflow run build-arm.yml

# 2. Wait for completion and download
gh run download <RUN_ID> -n linera-arm64

# 3. Copy to Termux
scp -P 8022 linera u0_a319@192.168.18.4:~/bin/linera
chmod +x ~/bin/linera
```

---

### Phase 2: REST API Server

#### [NEW] [Cargo.toml](file:///home/uratmangun/CascadeProjects/linera-termux/Cargo.toml)

Rust project configuration with dependencies:
- `axum` - REST API framework
- `tokio` - Async runtime
- `serde` / `serde_json` - JSON handling
- `reqwest` - HTTP client for GraphQL proxy
- `tracing` - Logging

---

#### [NEW] [src/main.rs](file:///home/uratmangun/CascadeProjects/linera-termux/src/main.rs)

Main entry point with Axum router setup:
```rust
// Endpoints:
// POST /service/start  -> start_service()
// POST /service/stop   -> stop_service()
// GET  /service/status -> get_status()
// POST /wallet/init    -> init_wallet()
// POST /owner/add      -> add_owner()
// POST /graphql        -> proxy_graphql()
```

---

#### [NEW] [src/linera_manager.rs](file:///home/uratmangun/CascadeProjects/linera-termux/src/linera_manager.rs)

Core logic for managing linera processes:
- `start_service()` - Spawn `linera service --port 8080`
- `stop_service()` - Kill the running process
- `init_wallet()` - Run `linera wallet init --faucet <url>`
- `add_owner()` - Run `linera change-ownership --owner-public-keys ...`
- Process ID tracking and status monitoring

---

#### [NEW] [src/graphql_proxy.rs](file:///home/uratmangun/CascadeProjects/linera-termux/src/graphql_proxy.rs)

GraphQL proxy implementation:
- Forward POST requests to `http://localhost:8080/chains/<chain>/applications/<app>`
- Return GraphQL responses

---

### API Specification

| Endpoint | Method | Request Body | Response |
|----------|--------|--------------|----------|
| `/service/start` | POST | `{ "port": 8080, "faucet_url": "..." }` | `{ "status": "started" }` |
| `/service/stop` | POST | - | `{ "status": "stopped" }` |
| `/service/status` | GET | - | `{ "running": true, "pid": 1234 }` |
| `/wallet/init` | POST | `{ "faucet_url": "..." }` | `{ "chain_id": "...", "public_key": "..." }` |
| `/owner/add` | POST | `{ "chain_id": "...", "public_keys": [...] }` | `{ "success": true }` |
| `/graphql` | POST | `{ "chain_id": "...", "app_id": "...", "query": "..." }` | GraphQL response |

---

## Verification Plan

### Automated Tests

#### GitHub Actions Build Verification
```bash
# 1. Trigger the workflow from your repo
gh workflow run build-arm.yml

# 2. Watch the build progress
gh run watch

# 3. Download the artifact once complete
gh run download --name linera-arm64

# 4. Copy to Termux
scp -P 8022 linera u0_a319@192.168.18.4:~/bin/

# 5. Verify on Termux
ssh -p 8022 u0_a319@192.168.18.4 "chmod +x ~/bin/linera && ~/bin/linera --version"
```

#### REST Server Build
```bash
# In the project directory
cd /path/to/linera-termux
cargo build --release

# Verify binary
ls -la target/release/linera-rest-server
```

### Manual Verification

#### Test 1: Start/Stop Service
1. Start the REST server: `./linera-rest-server`
2. In another terminal: `curl -X POST http://localhost:3000/service/start -H "Content-Type: application/json" -d '{"port": 8080}'`
3. Check status: `curl http://localhost:3000/service/status`
4. Verify linera service is running: `curl http://localhost:8080/`
5. Stop: `curl -X POST http://localhost:3000/service/stop`

#### Test 2: Wallet Initialization
1. Ensure service is started
2. Init wallet: `curl -X POST http://localhost:3000/wallet/init -H "Content-Type: application/json" -d '{"faucet_url": "https://faucet.testnet.linera.net"}'`
3. Verify response contains `chain_id` and `public_key`

#### Test 3: Add Owner
1. Generate a new public key (from another wallet)
2. Call add owner endpoint with the public key
3. Verify the ownership change via GraphQL

#### Test 4: GraphQL Proxy
1. Query via REST: `curl -X POST http://localhost:3000/graphql -H "Content-Type: application/json" -d '{"chain_id": "...", "app_id": "...", "query": "{ __schema { types { name } } }"}'`
2. Verify response matches direct GraphQL query

---

## File Structure

```
linera-termux/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, router setup
│   ├── linera_manager.rs    # Process management
│   ├── graphql_proxy.rs     # GraphQL forwarding
│   └── models.rs            # Request/Response types
├── scripts/
│   ├── install-deps.sh      # Termux dependency installation
│   └── build-linera.sh      # Linera compilation script
└── README.md                # Usage documentation
```

---

## Questions for User

1. **Faucet URL**: Do you have a specific Linera testnet faucet URL to use, or should I use the default testnet faucet?

2. **Persistence**: Are you okay with memory-only storage (data lost on restart), or do you need RocksDB persistence (more complex compilation)?

3. **Authentication**: Should the REST API have authentication (API key) to prevent unauthorized access?
