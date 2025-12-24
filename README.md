# Linera REST API Server for Termux

A Rust REST API server that wraps the Linera CLI, enabling wallet management and GraphQL proxying on Android/Termux.

## Features

- **Service Management**: Start/stop linera service via REST API
- **Wallet Operations**: Initialize wallet, get info, generate keypairs
- **Owner Management**: Add owners to multi-owner chains
- **GraphQL Proxy**: Forward queries to linera service

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/service/start` | POST | Start linera service |
| `/service/stop` | POST | Stop linera service |
| `/service/status` | GET | Get service status |
| `/wallet/init` | POST | Initialize wallet with faucet |
| `/wallet/info` | GET | Get wallet info |
| `/wallet/keygen` | POST | Generate new keypair |
| `/owner/add` | POST | Add owner to chain |
| `/graphql` | POST | Proxy GraphQL to chain/app |
| `/graphql/system` | POST | Proxy system GraphQL |
| `/health` | GET | Health check |

## Quick Start

### 1. Build the REST Server

```bash
cargo build --release
```

### 2. Set Environment Variables

```bash
export LINERA_BIN=~/bin/linera
export LINERA_WALLET=~/linera-wallet.json
export LINERA_KEYSTORE=~/linera-keystore.json
export PORT=3000
```

### 3. Run the Server

```bash
./target/release/linera-rest-server
```

## Usage Examples

### Initialize Wallet

```bash
curl -X POST http://localhost:3000/wallet/init \
  -H "Content-Type: application/json" \
  -d '{"faucet_url": "https://faucet.testnet-conway.linera.net"}'
```

### Start Linera Service

```bash
curl -X POST http://localhost:3000/service/start \
  -H "Content-Type: application/json" \
  -d '{"port": 8080}'
```

### Add Owner to Chain

```bash
curl -X POST http://localhost:3000/owner/add \
  -H "Content-Type: application/json" \
  -d '{
    "chain_id": "your-chain-id",
    "public_keys": ["owner-public-key-1", "owner-public-key-2"]
  }'
```

### Query GraphQL

```bash
curl -X POST http://localhost:3000/graphql \
  -H "Content-Type: application/json" \
  -d '{
    "chain_id": "your-chain-id",
    "app_id": "your-app-id",
    "query": "{ __schema { types { name } } }"
  }'
```

## License

MIT
