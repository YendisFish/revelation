mkdir -p initramfs/{proc,sys,dev}
cp init initramfs/init

cd initramfs
find . -print0 | cpio -o -H newc -0 > ../initramfs.cpio
cd ..
echo "Initramfs assembled"

echo "Verifying initramfs"
if [ -f initramfs.cpio ] && [ -s initramfs.cpio ]; then
    echo "Initramfs verification passed"
else
    echo "initramfs.cpio doesn't exist or is size 0. ruh roh!"
fi
