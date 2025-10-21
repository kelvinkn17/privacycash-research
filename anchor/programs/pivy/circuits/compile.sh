#!/bin/bash

# PIVY Circuit Compilation Script
# This compiles all circuits and generates the necessary artifacts

set -e

echo "========================================="
echo "  PIVY Circuit Compilation"
echo "========================================="

# Colors for output
GREEN='\033[0.32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if circom is installed
if ! command -v circom &> /dev/null; then
    echo -e "${RED}Error: circom is not installed${NC}"
    echo "Install it with: npm install -g circom"
    exit 1
fi

# Check if snarkjs is installed
if ! command -v snarkjs &> /dev/null; then
    echo -e "${RED}Error: snarkjs is not installed${NC}"
    echo "Install it with: npm install -g snarkjs"
    exit 1
fi

# Create artifacts directory
mkdir -p ../../../artifacts/circuits/pivy

echo -e "${BLUE}Step 1: Compiling transaction_2x2 circuit...${NC}"
circom transaction_2x2.circom \
    --r1cs \
    --wasm \
    --sym \
    -o ../../../artifacts/circuits/pivy/ \
    -l ../../../node_modules

echo -e "${GREEN}✓ transaction_2x2 compiled${NC}"

echo -e "${BLUE}Step 2: Compiling bucket_withdrawal_partial (20 deposits with partial withdrawal) circuit...${NC}"
circom bucket_withdrawal_partial.circom \
    --r1cs \
    --wasm \
    --sym \
    -o ../../../artifacts/circuits/pivy/ \
    -l ../../../node_modules

echo -e "${GREEN}✓ bucket_withdrawal_partial compiled${NC}"

echo -e "${BLUE}Step 3: Compiling bucket_withdrawal_partial_50 (50 deposits with partial withdrawal) circuit...${NC}"
circom bucket_withdrawal_partial_50.circom \
    --r1cs \
    --wasm \
    --sym \
    -o ../../../artifacts/circuits/pivy/ \
    -l ../../../node_modules

echo -e "${GREEN}✓ bucket_withdrawal_partial_50 compiled${NC}"

echo ""
echo -e "${GREEN}========================================="
echo "  Compilation Complete!"
echo "=========================================${NC}"
echo ""
echo "Artifacts created in: ../../../artifacts/circuits/pivy/"
echo ""
echo "Circuits compiled:"
echo "  • transaction_2x2 - Standard operations (2 in, 2 out)"
echo "  • bucket_withdrawal_partial - Partial withdrawal (up to 20 deposits) ⭐ YOUR PRIORITY"
echo "  • bucket_withdrawal_partial_50 - Large partial withdrawal (up to 50 deposits)"
echo ""
echo "Next steps:"
echo "1. Download Powers of Tau: ./download_ptau.sh"
echo "2. Generate proving keys: ./setup_keys.sh"
echo "3. Run tests: cd ../../../tests && npm test"
echo ""
