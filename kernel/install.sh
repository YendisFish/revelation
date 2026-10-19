#!/bin/bash

cd ./kernel/

echo "Seeking for Alpine kernel..."
APK_NAME=$(curl -sL https://dl-cdn.alpinelinux.org/alpine/v3.20/main/x86_64/ | grep -o 'linux-virt-[0-9][^"]*\.apk' | head -n 1)
echo "Found kernel package: $APK_NAME"
echo "Installing kernel binary..."

mkdir -p "./extracted"
curl -L -O "https://dl-cdn.alpinelinux.org/alpine/v3.20/main/x86_64/${APK_NAME}"
gzip -dc "$APK_NAME" | tar -xvf - -C ./extracted/

cd ../
