// io.h
#ifndef IO_H
#define IO_H

#include <stdint.h>

// Write a byte to a hardware port
static inline void outb(uint16_t port, uint8_t val) {
    __asm__ __volatile__ ( "outb %0, %1" : : "a"(val), "Nd"(port) );
}

// Read a byte from a hardware port
static inline uint8_t inb(uint16_t port) {
    uint8_t ret;
    __asm__ __volatile__ ( "inb %1, %0" : "=a"(ret) : "Nd"(port) );
    return ret;
}

// Read a word (16 bits / 2 bytes) from a hardware port
static inline uint16_t inw(uint16_t port) {
    uint16_t ret;
    __asm__ __volatile__ ( "inw %1, %0" : "=a"(ret) : "Nd"(port) );
    return ret;
}

#endif
