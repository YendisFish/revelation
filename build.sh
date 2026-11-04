#!/bin/bash
docker build --platform linux/amd64 -t revelation .

./kernel/install.sh
docker run --rm -v "./build/:/build/" revelation
