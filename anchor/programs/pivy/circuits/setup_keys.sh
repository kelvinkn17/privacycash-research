#!/bin/bash

# PIVY Circuit Setup Script
# Generates proving and verification keys for circuits

set -e

echo "========================================="
echo "  PIVY Circuit Key Generation"
echo "========================================="

GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

ARTIFACTS_DIR="../../../artifacts/circuits/pivy"
PTAU_FILE="../../../artifacts/powersOfTau28_hez_final_16.ptau"

# Check if PowersOfTau file exists
if [ ! -f "$PTAU_FILE" ]; then
    echo -e "${RED}Error: Powers of Tau file not found${NC}"
    echo "Download it from: https://hermez.s3-eu-west-1.amazonaws.com/powersOfTau28_hez_final_16.ptau"
    echo "Or run: ./download_ptau.sh"
    exit 1
fi

echo -e "${BLUE}Step 1: Generating zkey for transaction_2x2...${NC}"
snarkjs groth16 setup \
    $ARTIFACTS_DIR/transaction_2x2.r1cs \
    $PTAU_FILE \
    $ARTIFACTS_DIR/transaction_2x2_0000.zkey

echo -e "${BLUE}Step 2: Contributing to transaction_2x2 zkey...${NC}"
echo "pivy-contribution" | snarkjs zkey contribute \
    $ARTIFACTS_DIR/transaction_2x2_0000.zkey \
    $ARTIFACTS_DIR/transaction_2x2_final.zkey \
    --name="PIVY First Contribution"

echo -e "${BLUE}Step 3: Exporting verification key for transaction_2x2...${NC}"
snarkjs zkey export verificationkey \
    $ARTIFACTS_DIR/transaction_2x2_final.zkey \
    $ARTIFACTS_DIR/transaction_2x2_vkey.json

echo -e "${GREEN}✓ transaction_2x2 keys generated${NC}"

echo -e "${BLUE}Step 4: Generating zkey for bucket_withdrawal_partial...${NC}"
snarkjs groth16 setup \
    $ARTIFACTS_DIR/bucket_withdrawal_partial.r1cs \
    $PTAU_FILE \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_0000.zkey

echo -e "${BLUE}Step 5: Contributing to bucket_withdrawal_partial zkey...${NC}"
echo "pivy-bucket-partial-contribution" | snarkjs zkey contribute \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_0000.zkey \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_final.zkey \
    --name="PIVY Bucket Partial Contribution"

echo -e "${BLUE}Step 6: Exporting verification key for bucket_withdrawal_partial...${NC}"
snarkjs zkey export verificationkey \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_final.zkey \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_vkey.json

echo -e "${GREEN}✓ bucket_withdrawal_partial keys generated${NC}"

echo -e "${BLUE}Step 7: Generating zkey for bucket_withdrawal_partial_50...${NC}"
snarkjs groth16 setup \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_50.r1cs \
    $PTAU_FILE \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_50_0000.zkey

echo -e "${BLUE}Step 8: Contributing to bucket_withdrawal_partial_50 zkey...${NC}"
echo "pivy-bucket-partial-50-contribution" | snarkjs zkey contribute \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_50_0000.zkey \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_50_final.zkey \
    --name="PIVY Bucket Partial 50 Contribution"

echo -e "${BLUE}Step 9: Exporting verification key for bucket_withdrawal_partial_50...${NC}"
snarkjs zkey export verificationkey \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_50_final.zkey \
    $ARTIFACTS_DIR/bucket_withdrawal_partial_50_vkey.json

echo -e "${GREEN}✓ bucket_withdrawal_partial_50 keys generated${NC}"

# Clean up intermediate files
rm $ARTIFACTS_DIR/*_0000.zkey

echo ""
echo -e "${GREEN}========================================="
echo "  Key Generation Complete!"
echo "=========================================${NC}"
echo ""
echo "Generated files:"
echo "  - transaction_2x2_final.zkey"
echo "  - transaction_2x2_vkey.json"
echo "  - bucket_withdrawal_partial_final.zkey ⭐"
echo "  - bucket_withdrawal_partial_vkey.json ⭐"
echo "  - bucket_withdrawal_partial_50_final.zkey"
echo "  - bucket_withdrawal_partial_50_vkey.json"
echo ""
echo "Next: Run tests to verify circuits work correctly"
echo ""
