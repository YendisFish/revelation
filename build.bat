@echo off
docker build --platform linux/amd64 -t revelation .
if errorlevel 1 exit /b %errorlevel%

call kernel\install.bat
if errorlevel 1 exit /b %errorlevel%

docker run --rm -v "%cd%\build:/build/" revelation