rm -rf ./initramfs

if [ -f init ]; then
    if file init | grep -q "ELF"; then
        rm init # THE BINARY, NOT THE SOURCE FILE.
    else
        echo "init is not an ELF binary, refusing to delete it. ruh roh!"
        exit 1
    fi
fi

rm -f *.cpio