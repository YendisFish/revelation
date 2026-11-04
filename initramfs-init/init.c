#include <sys/mount.h>
#include <sys/stat.h>
#include <unistd.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>

// Writes to kernel ring buffer (kmsg)
void print_kmsg(const char *msg) {
    int fd = open("/dev/kmsg", O_WRONLY);
    if (fd >= 0) {
        write(fd, msg, strlen(msg));
        close(fd);
    }
}

int main() {
    // These are required to talk to the kernel, and is just boilerplate from my understanding.
    mount("proc", "/proc", "proc", 0, NULL);
    mount("sysfs", "/sys", "sysfs", 0, NULL);
    mount("devtmpfs", "/dev", "devtmpfs", 0, NULL);

    // Link std io to QEMU serial console
    int console_fd = open("/dev/console", O_RDWR);
    if (console_fd >= 0) {
        dup2(console_fd, 0); // stdin
        dup2(console_fd, 1); // stdout
        dup2(console_fd, 2); // stderr
    }

    print_kmsg("<5>initramfs: Waiting for /dev/sda to show up...\n");

    // Wait for HDD to appear in /dev, then mount it to a temp dir.
    while (access("/dev/sda", F_OK) != 0) {
        sleep(1);
    }

    print_kmsg("<5>initramfs: /dev/sda found. Mounting to /sysroot...\n");

    mkdir("/sysroot", 0755);
    if (mount("/dev/sda", "/sysroot", "ext4", 0, NULL) != 0) {
        char error_msg[256];
        snprintf(error_msg, sizeof(error_msg), "<3>initramfs: Failed to mount /dev/sda to /sysroot: %s\n", strerror(errno));
        print_kmsg(error_msg);
    }

    // Move the virtual fs over to the new hdd.
    mount("/proc", "/sysroot/proc", NULL, MS_MOVE, NULL);
    mount("/sys", "/sysroot/sys", NULL, MS_MOVE, NULL);
    mount("/dev", "/sysroot/dev", NULL, MS_MOVE, NULL);

    // Change root to the new hdd.
    chroot("/sysroot");
    chdir(".");

    print_kmsg("<5>initramfs: Handing off to /bin/sh...\n");

    // Execute the real init system, genesis. At this point, this program is
    // wiped from memory, and PID 1 is transferred over to genesis.
    char *argv[] = {"/bin/sh", NULL};
    char *envp[] = {"PATH=/bin:/sbin:/usr/bin:/usr/sbin", NULL};

    // Eventually replace with /sbin/genesis when that's finished.
    // If you dont have an hdd image file with busybox installed at /bin/sh, then
    // it will panix at the end. This is expected and normal behavior.
    execve("/bin/sh", argv, envp);

    // Incase execve fails.
    // If it does, something went terrible to the 33rd degree.

    char error_msg[256];
    snprintf(error_msg, sizeof(error_msg), "<3>initramfs: FATAL - execve(\"/bin/sh\") failed with errno: %d (%s)", errno, strerror(errno));
    print_kmsg(error_msg);

    // Print to stderr just in case the console is active.
    fprintf(stderr, "INITRAMFS PANIC: Failed to execute main init!\n");
    fflush(stderr);

    // This stops a kernel panic and just hangs, so we don't ever risk PID 1 exiting.
    while (1) {
        sleep(1);
    }

    return 0;
}
