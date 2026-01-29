# Deployment Guide

This guide covers the deployment of the Remitwise Contracts suite to the Stellar network using Soroban.

## Prerequisites

- Soroban CLI installed
- Stellar account with sufficient XLM for deployment
- Rust toolchain for contract compilation
- Network access (Testnet or Mainnet)

## Contracts Overview

The Remitwise Contracts suite consists of four main contracts:

1. **Remittance Split** - Manages fund allocation percentages
2. **Bill Payments** - Handles bill creation and payment tracking
3. **Insurance** - Manages insurance policies and premiums
4. **Savings Goals** - Tracks savings goals and fund management

## Deployment Steps

### 1. Environment Setup

```bash
# Install Soroban CLI (if not already installed)
cargo install soroban-cli

# Configure network
soroban config network add testnet \
  --rpc-url https://soroban-testnet.stellar.org:443 \
  --network-passphrase "Test SDF Network ; September 2015"

soroban config network add mainnet \
  --rpc-url https://soroban-rpc.mainnet.stellar.org:443 \
  --network-passphrase "Public Global Stellar Network ; September 2015"
```

### 2. Build Contracts

```bash
# Build all contracts
cd bill_payments
soroban contract build

cd ../insurance
soroban contract build

cd ../remittance_split
soroban contract build

cd ../savings_goals
soroban contract build
```

### 3. Deploy to Testnet

#### Create Deployer Identity

```bash
# Create or import deployer identity
soroban keys generate deployer
# Or import existing: soroban keys import deployer <secret_key>
```

#### Fund Deployer Account

```bash
# Get deployer address
soroban keys address deployer

# Fund the account using Stellar Laboratory or friendbot
# For testnet: https://laboratory.stellar.org/#account-creator?network=testnet
```

#### Deploy Contracts

```bash
# Set network
soroban config network testnet

# Deploy Remittance Split contract
cd remittance_split
REMittance_SPLIT_ID=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/remittance_split.wasm \
  --source deployer \
  --network testnet)

echo "Remittance Split deployed: $REMittance_SPLIT_ID"

# Deploy Bill Payments contract
cd ../bill_payments
BILL_PAYMENTS_ID=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/bill_payments.wasm \
  --source deployer \
  --network testnet)

echo "Bill Payments deployed: $BILL_PAYMENTS_ID"

# Deploy Insurance contract
cd ../insurance
INSURANCE_ID=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/insurance.wasm \
  --source deployer \
  --network testnet)

echo "Insurance deployed: $INSURANCE_ID"

# Deploy Savings Goals contract
cd ../savings_goals
SAVINGS_GOALS_ID=$(soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/savings_goals.wasm \
  --source deployer \
  --network testnet)

echo "Savings Goals deployed: $SAVINGS_GOALS_ID"
```

### 4. Initialize Contracts

#### Initialize Savings Goals

```bash
# Initialize storage
soroban contract invoke \
  --id $SAVINGS_GOALS_ID \
  --source deployer \
  --network testnet \
  -- \
  init
```

### 5. Configuration

#### Set Up Remittance Split (Example)

```bash
# Initialize split configuration for a user
# First, get user address
USER_ADDRESS="GA..."

# Initialize with 50% spending, 30% savings, 15% bills, 5% insurance
soroban contract invoke \
  --id $REMittance_SPLIT_ID \
  --source deployer \
  --network testnet \
  -- \
  initialize_split \
  --owner $USER_ADDRESS \
  --spending_percent 50 \
  --savings_percent 30 \
  --bills_percent 15 \
  --insurance_percent 5
```

## Network Configuration

### Testnet Configuration

- RPC URL: `https://soroban-testnet.stellar.org:443`
- Network Passphrase: `Test SDF Network ; September 2015`
- Friendbot: `https://friendbot.stellar.org`

### Mainnet Configuration

- RPC URL: `https://soroban-rpc.mainnet.stellar.org:443`
- Network Passphrase: `Public Global Stellar Network ; September 2015`

## Contract Addresses

After deployment, record the contract IDs:

```bash
# Save contract addresses to a file
cat > contract-addresses.txt << EOF
REMittance_SPLIT_ID=$REMittance_SPLIT_ID
BILL_PAYMENTS_ID=$BILL_PAYMENTS_ID
INSURANCE_ID=$INSURANCE_ID
SAVINGS_GOALS_ID=$SAVINGS_GOALS_ID
EOF
```

## Testing Deployment

### Basic Functionality Test

```bash
# Test remittance split calculation
soroban contract invoke \
  --id $REMittance_SPLIT_ID \
  --source deployer \
  --network testnet \
  -- \
  calculate_split \
  --total_amount 1000000000  # 100 XLM in stroops
```

### Integration Test

Create a complete user workflow:

1. Set up remittance split
2. Create savings goals
3. Create insurance policies
4. Create bills
5. Simulate remittance processing

## Troubleshooting

### Common Issues

#### Insufficient Funds

```
Error: insufficient funds
```

**Solution:** Ensure deployer account has enough XLM (at least 10 XLM recommended)

#### Build Failures

```
Error: failed to build contract
```

**Solution:** Check Rust toolchain and dependencies

```bash
rustup update
cargo clean
cargo build
```

#### Network Connection

```
Error: network error
```

**Solution:** Verify network configuration and internet connection

### Contract Verification

Verify deployed contracts:

```bash
# Check contract exists
soroban contract info --id $CONTRACT_ID --network testnet

# Test basic functionality
soroban contract invoke --id $CONTRACT_ID --network testnet -- get_split
```

## Production Deployment

For mainnet deployment:

1. Use mainnet network configuration
2. Fund deployer account with real XLM
3. Test thoroughly on testnet first
4. Consider multi-sig for deployer account
5. Document all contract addresses
6. Set up monitoring and alerts

## Cost Estimation

Approximate deployment costs (Testnet):

- Contract deployment: ~10 XLM per contract
- Storage operations: ~0.1 XLM per operation
- Function calls: ~0.01 XLM per call

## Maintenance

### Upgrading Contracts

1. Deploy new contract version
2. Migrate data if needed
3. Update client applications
4. Test thoroughly
5. Decommission old contract

### Monitoring

- Monitor contract storage usage
- Track function call volumes
- Set up alerts for failures
- Regular backup of contract states
