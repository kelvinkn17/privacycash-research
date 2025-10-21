#!/bin/bash

# Download Powers of Tau file (required for circuit setup)

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

PTAU_URL="https://hermez.s3-eu-west-1.amazonaws.com/powersOfTau28_hez_final_16.ptau"
PTAU_FILE="../../../artifacts/powersOfTau28_hez_final_16.ptau"

echo -e "${BLUE}Downloading Powers of Tau file...${NC}"
echo "This is a ~600MB file and may take a few minutes..."
echo ""

mkdir -p ../../../artifacts

if [ -f "$PTAU_FILE" ]; then
    echo -e "${GREEN}Powers of Tau file already exists!${NC}"
    exit 0
fi

curl -L -o "$PTAU_FILE" "$PTAU_URL"

echo ""
echo -e "${GREEN}✓ Download complete!${NC}"
echo "File saved to: $PTAU_FILE"
echo ""
echo "Next: Run ./setup_keys.sh to generate proving keys"
echo ""
