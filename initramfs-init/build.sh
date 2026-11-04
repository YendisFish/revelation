gcc -static init.c -o init
if [ $? -ne 0 ]; then
    echo "Build failed. ruh roh!"
    exit 1
fi
echo "Build complete"
echo "Checking is static"
ldd init
echo "Static check complete"