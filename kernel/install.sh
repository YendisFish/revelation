#!/bin/bash

cd ./kernel/ || exit 1

# echo "Seeking for Alpine kernel..."
# APK_NAME=$(curl -sL https://dl-cdn.alpinelinux.org/alpine/v3.20/main/x86_64/ | grep -o 'linux-virt-[0-9][^"]*\.apk' | head -n 1)
# echo "Found kernel package: $APK_NAME"
# echo "Installing kernel binary..."

# mkdir -p "./extracted"
# curl -L -O "https://dl-cdn.alpinelinux.org/alpine/v3.20/main/x86_64/${APK_NAME}"
# gzip -dc "$APK_NAME" | tar -xvf - -C ./extracted/

echo "Fetching Revelation kernel 7.3.1..."
curl -fsSL -o "boot-7.3.1-revelation.tar.gz" \
  "https://files.thcotd.org/mirror/revelation/kernel/boot-7.3.1-revelation.tar.gz?raw" \
  || { echo "Download failed"; exit 1; }
echo "Found kernel package"
echo "Extracting..."

mkdir -p "./extracted/boot"
tar -xzf "boot-7.3.1-revelation.tar.gz" -C ./extracted/boot
echo "Done!"

cd ../
