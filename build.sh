#!/bin/bash
docker build --platform linux/amd64 -t revelation .

docker run --rm -v "./build/:/build/" revelation
./kernel/install.sh
