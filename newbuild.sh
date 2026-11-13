#!/bin/bash
set -e

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

# 1. Ensure Revelation kernel binary is downloaded and extracted
if [ ! -f "./kernel/extracted/boot/vmlinuz-7.3.1-revelation" ]; then
    echo "=> Revelation kernel not found. Running ./kernel/install.sh..."
    ./kernel/install.sh
fi

# 2. Ensure git submodules (lwext4 for beginning bootloader) are initialized
if [ ! -f "./genesis/beginning/bios/lib/lwext4/src/ext4.c" ]; then
    echo "=> Initializing submodules (lwext4 for beginning bootloader)..."
    git submodule update --init --recursive
fi

# 3. Ensure base docker image 'revelation' exists
if ! docker image inspect revelation:latest >/dev/null 2>&1; then
    echo "=> Building base revelation Docker image..."
    docker build --platform linux/amd64 -t revelation .
fi

# 4. Ensure builder image 'revelation-builder' (with python3 and nasm) exists
if ! docker image inspect revelation-builder:latest >/dev/null 2>&1; then
    echo "=> Setting up revelation-builder Docker environment..."
    docker build --platform linux/amd64 -t revelation-builder - <<'EOF'
FROM revelation
RUN apk update && apk add --no-cache python3 nasm
WORKDIR /bld
ENTRYPOINT ["/usr/bin/python3", "/bld/hardhat/main.py"]
EOF
fi

# Ensure host ./build directory exists
mkdir -p "$REPO_DIR/build"

# 5. Run Hardhat in container with persistent build caching and current repo mounted
echo "=> Invoking Hardhat build system..."
docker run --rm --platform linux/amd64 \
    -v "$REPO_DIR/build:/build" \
    -v "$REPO_DIR:/bld" \
    revelation-builder "$@"
