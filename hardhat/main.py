#!/usr/bin/env python3
import shutil
import subprocess
from logging import NullHandler
import os
import argparse

class Color:
    RESET   = "\033[0m"
    BOLD    = "\033[1m"
    DIM     = "\033[2m"

    RED     = "\033[31m"
    GREEN   = "\033[32m"
    YELLOW  = "\033[33m"
    BLUE    = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN    = "\033[36m"
    WHITE   = "\033[37m"

def bold(string: str) -> str:
    return f"{Color.BOLD}{string}{Color.RESET}"
def dim(string: str) -> str:
    return f"{Color.DIM}{string}{Color.RESET}"
def red(string: str) -> str:
    return f"{Color.RED}{string}{Color.RESET}"
def green(string: str) -> str:
    return f"{Color.GREEN}{string}{Color.RESET}"
def yellow(string: str) -> str:
    return f"{Color.YELLOW}{string}{Color.RESET}"
def blue(string: str) -> str:
    return f"{Color.BLUE}{string}{Color.RESET}"
def magenta(string: str) -> str:
    return f"{Color.MAGENTA}{string}{Color.RESET}"
def cyan(string: str) -> str:
    return f"{Color.CYAN}{string}{Color.RESET}"
def white(string: str) -> str:
    return f"{Color.WHITE}{string}{Color.RESET}"

# quick wrapper class over print to make things pretty
class Logger:
    def header(self, content: str):
        print(content)
    def info(self, content: str):
        print(green("=> ") + content) 
    def error(self, content: str):
        print(red("=> [ERROR] ") + content)


logger = Logger()

help_msg = f"""
{Color.BLUE}{Color.BOLD}Hardhat{Color.RESET} - Revelation build system

Standard usage - hardhat -s full

Build Steps:
    - {Color.BLUE}full{Color.RESET}: removes old artifacts and rebuilds everything
    - {Color.BLUE}ospkg{Color.RESET}: rebuilds initramfs
    - {Color.BLUE}fspkg{Color.RESET}: rebuilds diskfs
    - {Color.BLUE}genesis{Color.RESET}: rebuilds genesis init system
    - {Color.BLUE}phoebe{Color.RESET}: rebuilds phoebe greeter
    - {Color.BLUE}userspace{Color.RESET}: rebuilds all userspace applications
 """

parser = argparse.ArgumentParser(
    prog="hardhat",
    description="Hardhat is the build system for Revelation",
    add_help=False,
)

parser.add_argument("-h", "--help", action="store_true")
parser.add_argument(
    "-s", "--steps",
    action="append",
    default=[],                    
)

def sh(cmd: str):
    subprocess.run(cmd.split(), check=True)

def main() -> None:
    args = parser.parse_args()

    if args.help:
        print(help_msg)
        exit()

    steps = []
    if args.steps:
        # split up comma delimited steps
        for raw in args.steps:
            steps.extend(s.strip() for s in raw.split(",") if s.strip())

    if "full" in steps:
        build_all()
        
            
    print(steps)
        

paths = {
    "OS": "/build/ospkg",
    "FS": "/build/fspkg",
}

def build_all():
    logger.info("performing full build")

    logger.info("removing old build artifacts")
    shutil.rmtree(paths["OS"], ignore_errors=True)
    shutil.rmtree(paths["FS"], ignore_errors=True)

if __name__ == "__main__":
    main()


