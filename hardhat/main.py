#!/usr/bin/env python3
"""
Hardhat - Revelation Build System
Reimplementation of container.sh and disk assembly in Python.

Abides by guidelines:
- Explicit error handling (no unhandled exceptions)
- Idiomatic and heavily commented
- Clean separation of build steps and dependency tracking
- Caches binary builds in /build/target
- Assembles the beginning bootloader, kernel, and genesis into a bootable revelation.img
"""

import argparse
import glob
import gzip
import os
import shutil
import struct
import subprocess
import sys
from typing import List, Optional


class Color:
    """ANSI color codes for pretty terminal logging."""
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


def cyan(string: str) -> str:
    return f"{Color.CYAN}{string}{Color.RESET}"


class Logger:
    """Structured terminal logging to match hardhat conventions."""
    def header(self, content: str) -> None:
        print(f"\n{bold(blue('=== ' + content + ' ==='))}")

    def info(self, content: str) -> None:
        print(green("=> ") + content)

    def warn(self, content: str) -> None:
        print(yellow("=> [WARN] ") + content)

    def error(self, content: str) -> None:
        print(red("=> [ERROR] ") + content, file=sys.stderr)

    def success(self, content: str) -> None:
        print(bold(green("=> [SUCCESS] ")) + content)


logger = Logger()


class Config:
    """
    Central build configuration and filesystem layout.
    Defaults to container paths (/bld and /build), with fallback for host runs.
    """
    # Root of source repository
    REPO_ROOT: str = os.environ.get(
        "REVELATION_ROOT",
        "/bld" if os.path.exists("/bld") else os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    )

    # Output directory (mounted from host ./build to /build in container)
    BUILD_DIR: str = os.environ.get(
        "REVELATION_BUILD",
        "/build" if os.path.exists("/build") else os.path.join(REPO_ROOT, "build")
    )

    # Subdirectories for staging
    OS_DIR: str = os.path.join(BUILD_DIR, "ospkg")          # initramfs staging
    FS_DIR: str = os.path.join(BUILD_DIR, "fspkg")          # diskfs staging
    TARGET_DIR: str = os.path.join(BUILD_DIR, "target")     # Rust build cache
    CARGO_HOME: str = os.path.join(BUILD_DIR, "cargo")      # Cargo crates cache
    BOOTLOADER_BUILD: str = os.path.join(BUILD_DIR, "bootloader")

    # Output disk image and initramfs files
    REVELATION_IMG: str = os.path.join(BUILD_DIR, "revelation.img")
    INITRAMFS_CPIO: str = os.path.join(BUILD_DIR, "initramfs.cpio")
    INITRAMFS_CPIO_GZ: str = os.path.join(BUILD_DIR, "initramfs.cpio.gz")

    # Source paths
    GENESIS_SRC: str = os.path.join(REPO_ROOT, "genesis")
    PHOEBE_SRC: str = os.path.join(REPO_ROOT, "phoebe")
    BEGINNING_SRC: str = os.path.join(REPO_ROOT, "genesis", "beginning", "bios")
    KERNEL_BOOT_SRC: str = os.path.join(REPO_ROOT, "kernel", "extracted", "boot")
    KERNEL_MODS_SRC: str = os.path.join(
        REPO_ROOT, "kernel", "extracted", "lib", "modules", "6.6.142-0-virt"
    )

    # Partition and filesystem settings
    PARTUUID: str = "12345678-ABCD-EF01-2345-6789ABCDEF01"
    SECTOR_SIZE: int = 512
    FS_START_SECTOR: int = 2048  # 1MiB offset
    ROOTFS_SIZE_MIB: int = 990   # Size of ext4 rootfs in MiB
    TOTAL_DISK_SIZE_MIB: int = 1024  # 1GiB total disk image


def run_command(
    cmd: List[str],
    cwd: Optional[str] = None,
    env: Optional[dict] = None,
    capture_output: bool = False
) -> subprocess.CompletedProcess:
    """
    Safely execute a subprocess command with proper error reporting.
    Does not allow exceptions to bubble unhandled; prints context on failure.
    """
    cmd_str = " ".join(cmd)
    logger.info(f"Running: {dim(cmd_str)}" + (f" in {cwd}" if cwd else ""))

    # Merge environment with existing environment
    run_env = os.environ.copy()
    if env:
        run_env.update(env)

    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            env=run_env,
            check=True,
            stdout=subprocess.PIPE if capture_output else None,
            stderr=subprocess.PIPE if capture_output else None,
            text=True if capture_output else False
        )
        return proc
    except subprocess.CalledProcessError as err:
        logger.error(f"Command failed with exit code {err.returncode}: {cmd_str}")
        if capture_output and err.stderr:
            logger.error(f"Output: {err.stderr}")
        raise RuntimeError(f"Step failed: {cmd_str}") from err
    except FileNotFoundError as err:
        logger.error(f"Binary not found: {cmd[0]}")
        raise RuntimeError(f"Missing required tool: {cmd[0]}") from err


def ensure_dirs() -> None:
    """Ensure base output directories exist."""
    try:
        os.makedirs(Config.BUILD_DIR, exist_ok=True)
        os.makedirs(Config.TARGET_DIR, exist_ok=True)
        os.makedirs(Config.BOOTLOADER_BUILD, exist_ok=True)
    except OSError as err:
        logger.error(f"Failed to create build directories: {err}")
        raise


def clean_ospkg() -> None:
    """Remove existing initramfs staging directory."""
    logger.info("Cleaning ospkg staging directory")
    try:
        if os.path.exists(Config.OS_DIR):
            shutil.rmtree(Config.OS_DIR)
        os.makedirs(Config.OS_DIR, exist_ok=True)
    except OSError as err:
        logger.error(f"Failed to clean ospkg: {err}")
        raise


def clean_fspkg() -> None:
    """Remove existing rootfs staging directory."""
    logger.info("Cleaning fspkg staging directory")
    try:
        if os.path.exists(Config.FS_DIR):
            shutil.rmtree(Config.FS_DIR)
        os.makedirs(Config.FS_DIR, exist_ok=True)
    except OSError as err:
        logger.error(f"Failed to clean fspkg: {err}")
        raise


def clean_all(clean_target: bool = False) -> None:
    """Remove all staging directories and optionally the Rust binary cache."""
    logger.header("Cleaning build artifacts")
    clean_ospkg()
    clean_fspkg()

    if clean_target:
        logger.info("Wiping Rust binary cache (/build/target)")
        if os.path.exists(Config.TARGET_DIR):
            shutil.rmtree(Config.TARGET_DIR, ignore_errors=True)

    if os.path.exists(Config.BOOTLOADER_BUILD):
        shutil.rmtree(Config.BOOTLOADER_BUILD, ignore_errors=True)


def prepare_kernel_modules() -> None:
    """
    Extract compressed kernel modules (*.ko.gz) and stage them into $OS/lib/modules/.
    Matches container.sh behavior for kernel module packaging.
    """
    logger.info("Preparing kernel modules for initramfs")
    dest_dir = os.path.join(Config.OS_DIR, "lib", "modules")
    os.makedirs(dest_dir, exist_ok=True)

    if not os.path.exists(Config.KERNEL_MODS_SRC):
        logger.warn(f"Kernel modules directory not found: {Config.KERNEL_MODS_SRC}")
        return

    # Decompress any .ko.gz files in place or copy uncompressed modules
    for root, _, files in os.walk(Config.KERNEL_MODS_SRC):
        for file in files:
            src_file = os.path.join(root, file)
            # Compute relative path inside the modules tree
            rel_path = os.path.relpath(src_file, Config.KERNEL_MODS_SRC)

            if file.endswith(".ko.gz"):
                # Target path without .gz
                dest_file = os.path.join(dest_dir, rel_path[:-3])
                os.makedirs(os.path.dirname(dest_file), exist_ok=True)
                try:
                    with gzip.open(src_file, "rb") as f_in:
                        with open(dest_file, "wb") as f_out:
                            shutil.copyfileobj(f_in, f_out)
                except OSError as err:
                    logger.error(f"Failed decompressing module {src_file}: {err}")
            else:
                dest_file = os.path.join(dest_dir, rel_path)
                os.makedirs(os.path.dirname(dest_file), exist_ok=True)
                try:
                    shutil.copy2(src_file, dest_file)
                except OSError as err:
                    logger.error(f"Failed copying module {src_file}: {err}")


def create_ramfs_structure() -> None:
    """Create essential mount points in ramfs staging."""
    logger.info("Creating ramfs directories in ospkg (dev, sys, proc, lib)")
    for d in ["dev", "sys", "proc", "lib"]:
        os.makedirs(os.path.join(Config.OS_DIR, d), exist_ok=True)


def create_diskfs_structure() -> None:
    """Create standard filesystem hierarchy in diskfs staging."""
    logger.info("Creating diskfs directories in fspkg (run, bin, apps, etc, lib)")
    for d in ["run", "bin", "apps", "etc", "lib"]:
        os.makedirs(os.path.join(Config.FS_DIR, d), exist_ok=True)


def build_genesis() -> None:
    """
    Build the genesis init system using Cargo.
    Caches build output in /build/target so rebuilds are fast.
    Copies binary to $OS/init, boot.toml to $OS/, and genesis.toml to $FS/.
    """
    logger.header("Building Genesis Init System")

    # Use CARGO_TARGET_DIR and CARGO_HOME to cache in /build
    env = {
        "CARGO_TARGET_DIR": Config.TARGET_DIR,
        "CARGO_HOME": Config.CARGO_HOME,
    }
    cargo_cmd = [
        "cargo", "build", "--release",
        "--manifest-path", os.path.join(Config.GENESIS_SRC, "Cargo.toml"),
        "--target-dir", Config.TARGET_DIR
    ]
    run_command(cargo_cmd, cwd=Config.GENESIS_SRC, env=env)

    # Binary output location
    bin_src = os.path.join(Config.TARGET_DIR, "release", "genesis")
    if not os.path.exists(bin_src):
        raise FileNotFoundError(f"Expected genesis binary not found at {bin_src}")

    # Copy to initramfs as /init
    init_dest = os.path.join(Config.OS_DIR, "init")
    logger.info(f"Installing {bin_src} -> {init_dest}")
    shutil.copy2(bin_src, init_dest)
    os.chmod(init_dest, 0o755)

    # Copy configuration files
    boot_toml = os.path.join(Config.GENESIS_SRC, "boot.toml")
    genesis_toml = os.path.join(Config.GENESIS_SRC, "genesis.toml")

    if os.path.exists(boot_toml):
        logger.info(f"Copying {boot_toml} -> {Config.OS_DIR}/boot.toml")
        shutil.copy2(boot_toml, os.path.join(Config.OS_DIR, "boot.toml"))
    else:
        logger.warn(f"boot.toml not found at {boot_toml}")

    if os.path.exists(genesis_toml):
        logger.info(f"Copying {genesis_toml} -> {Config.FS_DIR}/genesis.toml")
        shutil.copy2(genesis_toml, os.path.join(Config.FS_DIR, "genesis.toml"))
    else:
        logger.warn(f"genesis.toml not found at {genesis_toml}")


def build_phoebe() -> None:
    """
    Build the phoebe greeter using Cargo.
    Caches build output in /build/target.
    Copies binary to $FS/bin/phoebe.
    """
    logger.header("Building Phoebe Greeter")

    env = {
        "CARGO_TARGET_DIR": Config.TARGET_DIR,
        "CARGO_HOME": Config.CARGO_HOME,
    }
    cargo_cmd = [
        "cargo", "build", "--release",
        "--manifest-path", os.path.join(Config.PHOEBE_SRC, "Cargo.toml"),
        "--target-dir", Config.TARGET_DIR
    ]
    run_command(cargo_cmd, cwd=Config.PHOEBE_SRC, env=env)

    bin_src = os.path.join(Config.TARGET_DIR, "release", "phoebe")
    if not os.path.exists(bin_src):
        raise FileNotFoundError(f"Expected phoebe binary not found at {bin_src}")

    dest_bin = os.path.join(Config.FS_DIR, "bin", "phoebe")
    os.makedirs(os.path.dirname(dest_bin), exist_ok=True)
    logger.info(f"Installing {bin_src} -> {dest_bin}")
    shutil.copy2(bin_src, dest_bin)
    os.chmod(dest_bin, 0o755)


def create_initramfs() -> None:
    """
    Package $OS into a bootable cpio archive.
    Generates both uncompressed initramfs.cpio (for beginning)
    and initramfs.cpio.gz (standard compressed).
    """
    logger.header("Packaging initramfs")

    # Generate uncompressed cpio using cpio binary
    # find . -print0 | cpio --null --create --format=newc > /build/initramfs.cpio
    cmd = (
        f"find . -print0 | cpio --null --create --format=newc > '{Config.INITRAMFS_CPIO}'"
    )
    logger.info(f"Archiving {Config.OS_DIR} to {Config.INITRAMFS_CPIO}")
    subprocess.run(cmd, shell=True, cwd=Config.OS_DIR, check=True)

    # Also generate gzipped version
    logger.info(f"Compressing to {Config.INITRAMFS_CPIO_GZ}")
    with open(Config.INITRAMFS_CPIO, "rb") as f_in:
        with gzip.open(Config.INITRAMFS_CPIO_GZ, "wb", compresslevel=9) as f_out:
            shutil.copyfileobj(f_in, f_out)

    # Copy initramfs into fspkg so beginning bootloader can load it from ext4
    fs_initramfs = os.path.join(Config.FS_DIR, "initramfs.cpio")
    logger.info(f"Copying {Config.INITRAMFS_CPIO} -> {fs_initramfs}")
    shutil.copy2(Config.INITRAMFS_CPIO, fs_initramfs)


def copy_kernel_image() -> None:
    """
    Locate the Revelation kernel binary (vmlinuz-7.3.1-revelation)
    and copy it to $FS/vmlinuz so the beginning bootloader can load it.
    """
    logger.info("Staging kernel binary into diskfs (/vmlinuz)")
    vmlinuz_candidates = glob.glob(os.path.join(Config.KERNEL_BOOT_SRC, "vmlinuz*"))
    if not vmlinuz_candidates:
        raise FileNotFoundError(f"No kernel image found in {Config.KERNEL_BOOT_SRC}")

    # Prioritize 7.3.1-revelation if present
    kernel_src = None
    for cand in vmlinuz_candidates:
        if "7.3.1-revelation" in cand:
            kernel_src = cand
            break
    if not kernel_src:
        kernel_src = vmlinuz_candidates[0]

    dest_vmlinuz = os.path.join(Config.FS_DIR, "vmlinuz")
    logger.info(f"Copying {kernel_src} -> {dest_vmlinuz}")
    shutil.copy2(kernel_src, dest_vmlinuz)


def build_beginning_bootloader() -> dict:
    """
    Compile the beginning BIOS bootloader stages:
    1. boot.asm -> boot.bin (512-byte MBR stage 1)
    2. C sources + lwext4 -> kernel.bin (32-bit protected mode loader)
    3. stage2.asm -> stage2.bin (with KERNEL_SECTORS defined)
    Returns dictionary with paths to boot.bin, stage2.bin, and kernel.bin.
    """
    logger.header("Building Beginning Bootloader (BIOS)")

    if not os.path.exists(Config.BEGINNING_SRC):
        raise FileNotFoundError(f"Beginning bootloader source not found: {Config.BEGINNING_SRC}")

    bld_dir = Config.BOOTLOADER_BUILD
    os.makedirs(bld_dir, exist_ok=True)

    boot_bin = os.path.join(bld_dir, "boot.bin")
    stage2_bin = os.path.join(bld_dir, "stage2.bin")
    kernel_bin = os.path.join(bld_dir, "kernel.bin")

    # Step 1: Compile boot.asm -> boot.bin
    logger.info("Assembling boot.asm (MBR Stage 1)")
    run_command(
        ["nasm", "-f", "bin", "boot.asm", "-o", boot_bin],
        cwd=Config.BEGINNING_SRC
    )
    if os.path.getsize(boot_bin) != Config.SECTOR_SIZE:
        raise ValueError(f"boot.bin must be exactly {Config.SECTOR_SIZE} bytes")

    # Step 2: Compile C sources and lwext4
    # Resolve GCC's freestanding stdint-gcc.h to prevent musl 64-bit headers
    # from overriding 32-bit types (which would otherwise define uint64_t as 32-bit unsigned long)
    stdint_res = subprocess.run(
        ["gcc", "-print-file-name=include/stdint-gcc.h"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    stdint_gcc = stdint_res.stdout.strip()
    stdint_args = ["-D_STDINT_H", "-include", stdint_gcc] if stdint_gcc and os.path.exists(stdint_gcc) else []

    # Freestanding 64-bit division routines (__udivdi3, __umoddi3, __udivmoddi4)
    # required by 32-bit lwext4 when freestanding without 32-bit libgcc.a
    div64_c = os.path.join(bld_dir, "div64.c")
    div64_o = os.path.join(bld_dir, "div64.o")
    div64_code = (
        "typedef unsigned long long uint64_t;\n\n"
        "uint64_t __udivmoddi4(uint64_t n, uint64_t d, uint64_t *r) {\n"
        "    if (d == 0) return 0;\n"
        "    uint64_t q = 0;\n"
        "    for (int i = 63; i >= 0; i--) {\n"
        "        if ((n >> i) >= d) {\n"
        "            n -= (d << i);\n"
        "            q |= (1ULL << i);\n"
        "        }\n"
        "    }\n"
        "    if (r) *r = n;\n"
        "    return q;\n"
        "}\n\n"
        "uint64_t __udivdi3(uint64_t n, uint64_t d) {\n"
        "    return __udivmoddi4(n, d, 0);\n"
        "}\n\n"
        "uint64_t __umoddi3(uint64_t n, uint64_t d) {\n"
        "    uint64_t r = 0;\n"
        "    __udivmoddi4(n, d, &r);\n"
        "    return r;\n"
        "}\n"
    )
    with open(div64_c, "w") as f:
        f.write(div64_code)

    if not os.path.exists(div64_o) or os.path.getmtime(div64_c) > os.path.getmtime(div64_o):
        run_command(["gcc", "-m32", "-march=i686", "-mgeneral-regs-only", "-ffreestanding", "-O2", "-nostdlib", "-c", div64_c, "-o", div64_o])

    c_sources = ["main.c", "ata.c", "ata_bdev.c", "libc.c"]
    lwext4_sources = glob.glob(os.path.join(Config.BEGINNING_SRC, "lib", "lwext4", "src", "*.c"))

    if not lwext4_sources:
        raise FileNotFoundError("lwext4 source files missing. Run git submodule update --init --recursive.")

    all_c_files = [os.path.join(Config.BEGINNING_SRC, f) for f in c_sources] + lwext4_sources

    cflags = [
        "-m32", "-march=i686", "-mgeneral-regs-only", "-mno-sse", "-mno-mmx", "-mno-sse2",
        "-ffreestanding", "-O2", "-nostdlib", "-Wall", "-Wextra",
        "-DCONFIG_USE_DEFAULT_CFG=1",
    ] + stdint_args + [
        f"-I{Config.BEGINNING_SRC}",
        f"-I{os.path.join(Config.BEGINNING_SRC, 'lib', 'lwext4', 'include')}",
        "-fno-pic", "-fno-pie", "-fno-stack-protector", "-U_FORTIFY_SOURCE",
        "-DCONFIG_DEBUG_PRINTF=0", "-DCONFIG_DEBUG_ASSERT=0"
    ]

    obj_files = [div64_o]
    logger.info(f"Compiling {len(all_c_files)} C source files for beginning...")
    for src in all_c_files:
        rel_base = os.path.splitext(os.path.basename(src))[0]
        # Distinguish lwext4 vs root objects
        prefix = "ext4_" if "lwext4" in src and not rel_base.startswith("ext4_") else ""
        obj_name = f"{prefix}{rel_base}.o"
        obj_path = os.path.join(bld_dir, obj_name)

        # Check if rebuild of this object is necessary
        if not os.path.exists(obj_path) or os.path.getmtime(src) > os.path.getmtime(obj_path):
            compile_cmd = ["gcc"] + cflags + ["-c", src, "-o", obj_path]
            run_command(compile_cmd, cwd=Config.BEGINNING_SRC)

        obj_files.append(obj_path)

    # Step 3: Link kernel.bin using linker.ld
    logger.info("Linking kernel.bin")
    linker_script = os.path.join(Config.BEGINNING_SRC, "linker.ld")
    link_cmd = [
        "gcc", "-T", linker_script, "-m32", "-nostdlib",
        "-ffreestanding", "-no-pie", "-Wl,--build-id=none",
        "-o", kernel_bin
    ] + obj_files
    run_command(link_cmd, cwd=Config.BEGINNING_SRC)

    # Determine size and sector count of kernel.bin
    kernel_size = os.path.getsize(kernel_bin)
    kernel_sectors = (kernel_size + Config.SECTOR_SIZE - 1) // Config.SECTOR_SIZE
    logger.info(f"kernel.bin size: {kernel_size} bytes ({kernel_sectors} sectors)")

    # Step 4: Compile stage2.asm with KERNEL_SECTORS defined
    logger.info(f"Assembling stage2.asm with KERNEL_SECTORS={kernel_sectors}")
    stage2_cmd = [
        "nasm", "-f", "bin",
        f"-DKERNEL_SECTORS={kernel_sectors}",
        "stage2.asm", "-o", stage2_bin
    ]
    run_command(stage2_cmd, cwd=Config.BEGINNING_SRC)

    stage2_size = os.path.getsize(stage2_bin)
    if stage2_size != 2048:
        logger.warn(f"stage2.bin size is {stage2_size} bytes (expected 2048 bytes / 4 sectors)")

    # Validate that bootloader does not overflow reserved sectors before sector 2048
    # stage2 is placed at sector 34 (4 sectors) and kernel.bin is placed at sector 38
    total_boot_bytes = Config.SECTOR_SIZE + stage2_size + kernel_size
    end_boot_sector = 38 + kernel_sectors
    if end_boot_sector > Config.FS_START_SECTOR:
        raise ValueError(
            f"Bootloader binary ends at sector {end_boot_sector}, exceeding "
            f"partition start sector {Config.FS_START_SECTOR}"
        )

    logger.success(f"Bootloader build complete ({total_boot_bytes} bytes total)")
    return {
        "boot_bin": boot_bin,
        "stage2_bin": stage2_bin,
        "kernel_bin": kernel_bin,
        "kernel_sectors": kernel_sectors
    }


def pack_and_assemble_disk(bootloader: Optional[dict] = None) -> None:
    """
    Assemble the complete bootable Revelation disk image:
    1. Build rootfs.ext4 containing all assets in $FS (using lwext4-compatible ext4 features).
    2. Create revelation.img with GPT partition table (GUID matching boot.toml).
    3. Copy rootfs.ext4 into partition 1 (starting at sector 2048).
    4. Write beginning bootloader (boot.bin, stage2.bin, kernel.bin) to sectors 0..N.
       Carefully preserves protective MBR partition table entries at offset 446 in sector 0!
    """
    logger.header("Assembling Bootable Disk Image")

    rootfs_ext4 = os.path.join(Config.BUILD_DIR, "rootfs.ext4")
    revelation_img = Config.REVELATION_IMG

    # Ensure required files are staged in FS
    if not os.path.exists(os.path.join(Config.FS_DIR, "vmlinuz")):
        copy_kernel_image()

    if not os.path.exists(os.path.join(Config.FS_DIR, "initramfs.cpio")):
        create_initramfs()

    # Step 1: Format ext4 rootfs from fspkg
    logger.info(f"Creating ext4 rootfs ({Config.ROOTFS_SIZE_MIB} MiB)")
    if os.path.exists(rootfs_ext4):
        os.remove(rootfs_ext4)

    # Pre-allocate sparse rootfs.ext4
    with open(rootfs_ext4, "wb") as f:
        f.truncate(Config.ROOTFS_SIZE_MIB * 1024 * 1024)

    # Format using mkfs.ext4 with features compatible with lwext4
    mkfs_cmd = [
        "mkfs.ext4",
        "-d", Config.FS_DIR,
        "-F",
        "-O", "^has_journal,^metadata_csum,^metadata_csum_seed,^orphan_file,^64bit",
        rootfs_ext4
    ]
    run_command(mkfs_cmd)

    # Step 2: Initialize total disk image and partition with sgdisk
    logger.info(f"Initializing {Config.TOTAL_DISK_SIZE_MIB} MiB disk image: {revelation_img}")
    with open(revelation_img, "wb") as f:
        f.truncate(Config.TOTAL_DISK_SIZE_MIB * 1024 * 1024)

    logger.info(f"Writing GPT partition table with PARTUUID {Config.PARTUUID}")
    sgdisk_cmd = [
        "sgdisk", "-o",
        f"-n", f"1:{Config.FS_START_SECTOR}:0",
        "-c", "1:rootfs",
        "-u", f"1:{Config.PARTUUID}",
        revelation_img
    ]
    run_command(sgdisk_cmd)

    # Step 3: Copy rootfs.ext4 into partition 1 at sector 2048 (1MiB offset)
    logger.info(f"Writing rootfs into {revelation_img} at sector {Config.FS_START_SECTOR}")
    offset = Config.FS_START_SECTOR * Config.SECTOR_SIZE
    with open(rootfs_ext4, "rb") as f_src:
        with open(revelation_img, "r+b") as f_dst:
            f_dst.seek(offset)
            shutil.copyfileobj(f_src, f_dst)

    # Remove temporary rootfs.ext4 to free space
    if os.path.exists(rootfs_ext4):
        os.remove(rootfs_ext4)

    # Step 4: Write beginning bootloader to reserved sectors
    if not bootloader:
        bootloader = build_beginning_bootloader()

    logger.info("Writing bootloader to disk sectors")
    with open(revelation_img, "r+b") as f_disk:
        # Read existing sector 0 to preserve partition table entries (offset 446 to 512)
        f_disk.seek(0)
        sector0_orig = f_disk.read(Config.SECTOR_SIZE)
        partition_table_bytes = sector0_orig[446:512]

        # Read boot.bin
        with open(bootloader["boot_bin"], "rb") as f_boot:
            boot_code = f_boot.read(446)

        # Write merged sector 0: boot code (0..445) + partition table (446..511)
        f_disk.seek(0)
        f_disk.write(boot_code + partition_table_bytes)

        # Write stage2.bin at sector 34 (offset 34 * 512 = 17408, safely past primary GPT)
        with open(bootloader["stage2_bin"], "rb") as f_stage2:
            stage2_data = f_stage2.read()
            f_disk.seek(34 * Config.SECTOR_SIZE)
            f_disk.write(stage2_data)

        # Write kernel.bin at sector 38 (offset 38 * 512 = 19456)
        with open(bootloader["kernel_bin"], "rb") as f_kernel:
            kernel_data = f_kernel.read()
            f_disk.seek(38 * Config.SECTOR_SIZE)
            f_disk.write(kernel_data)

    logger.success(f"Successfully assembled bootable disk image: {revelation_img}")
    logger.info(
        f"Boot with QEMU: "
        f"{bold('qemu-system-x86_64 -drive file=' + revelation_img + ',format=raw -serial stdio -m 512M')}"
    )


def build_full(clean_target: bool = False) -> None:
    """Execute full build pipeline."""
    logger.header("Starting Full Rebuild")
    clean_all(clean_target=clean_target)
    ensure_dirs()
    create_ramfs_structure()
    create_diskfs_structure()
    prepare_kernel_modules()

    # Compile Rust binaries into /build/target
    build_genesis()
    build_phoebe()

    # Stage assets and package initramfs
    create_initramfs()
    copy_kernel_image()

    # Build beginning bootloader and assemble disk image
    bootloader = build_beginning_bootloader()
    pack_and_assemble_disk(bootloader)
    logger.header("Full Build Finished Successfully")


help_msg = f"""
{Color.BLUE}{Color.BOLD}Hardhat{Color.RESET} - Revelation Build System

Usage:
    hardhat [OPTIONS]

Options:
    -s, --steps <step,step...>   Specify build step(s) to execute
    -c, --clean                  Clean staging directories before building
    --clean-all                  Clean all artifacts including Rust target cache
    -h, --help                   Display this help message

Build Steps:
    - {Color.BLUE}full{Color.RESET}: removes old packaging artifacts and rebuilds everything (default)
    - {Color.BLUE}beginning{Color.RESET}: rebuilds the beginning BIOS bootloader (boot.bin, stage2.bin, kernel.bin)
    - {Color.BLUE}genesis{Color.RESET}: rebuilds genesis init system (cached in /build/target)
    - {Color.BLUE}phoebe{Color.RESET}: rebuilds phoebe greeter (cached in /build/target)
    - {Color.BLUE}userspace{Color.RESET}: rebuilds all userspace applications
    - {Color.BLUE}ospkg{Color.RESET}: rebuilds ramfs staging and packages initramfs.cpio
    - {Color.BLUE}fspkg{Color.RESET}: rebuilds diskfs staging and prepares filesystem assets
    - {Color.BLUE}image{Color.RESET}: formats rootfs.ext4 and assembles the bootable revelation.img
    - {Color.BLUE}clean{Color.RESET}: removes staging artifacts
"""


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="hardhat",
        description="Hardhat is the build system for Revelation",
        add_help=False,
    )
    parser.add_argument("-h", "--help", action="store_true")
    parser.add_argument("-s", "--steps", action="append", default=[])
    parser.add_argument("-c", "--clean", action="store_true")
    parser.add_argument("--clean-all", action="store_true")

    args = parser.parse_args()

    if args.help:
        print(help_msg)
        sys.exit(0)

    # Parse comma-delimited steps
    steps: List[str] = []
    if args.steps:
        for raw in args.steps:
            steps.extend(s.strip().lower() for s in raw.split(",") if s.strip())

    # Default to full build if no specific step requested
    if not steps:
        steps = ["full"]

    ensure_dirs()

    if args.clean_all or "clean" in steps:
        clean_all(clean_target=args.clean_all)
        if "clean" in steps and len(steps) == 1:
            logger.success("Clean completed.")
            sys.exit(0)

    try:
        if "full" in steps:
            build_full(clean_target=args.clean_all)
            return

        # Granular step dispatch
        if args.clean or "ospkg" in steps:
            clean_ospkg()
            create_ramfs_structure()
            prepare_kernel_modules()

        if args.clean or "fspkg" in steps:
            clean_fspkg()
            create_diskfs_structure()

        if "genesis" in steps:
            create_ramfs_structure()
            build_genesis()

        if "phoebe" in steps or "userspace" in steps:
            create_diskfs_structure()
            build_phoebe()

        if "ospkg" in steps:
            # Ensure genesis binary is staged if not already present
            if not os.path.exists(os.path.join(Config.OS_DIR, "init")):
                build_genesis()
            create_initramfs()

        if "fspkg" in steps:
            if not os.path.exists(os.path.join(Config.FS_DIR, "bin", "phoebe")):
                build_phoebe()
            copy_kernel_image()
            if not os.path.exists(os.path.join(Config.FS_DIR, "initramfs.cpio")):
                create_initramfs()

        bootloader = None
        if "beginning" in steps or "bootloader" in steps:
            bootloader = build_beginning_bootloader()

        if "image" in steps:
            if not bootloader:
                bootloader = build_beginning_bootloader()
            pack_and_assemble_disk(bootloader)

        logger.success("Selected build steps completed successfully.")

    except Exception as err:
        logger.error(f"Build terminated due to error: {err}")
        sys.exit(1)


if __name__ == "__main__":
    main()
