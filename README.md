# Revelation
Revelation is a unique and revolutionary take on
declarative and uptime dependant Linux systems
and configurations. Utilizing Rust, containers,
type theory, and other techniques, Revelation acheives
a level of process safety that normally is not seen
in Linux systems.

### How it works
The first thing to know about revelation is that it
is very restrictive. There are certain things in a
system which make sense, and others which don't.

**Systems and Configs as Types**
Revelation treats the system as a singular, dynamic
type. The shape of the system dictates if programs
can safely run, in otherwords actually apply within
the type that your system outlines.

### How to Build
Running ``build.sh`` will build the OS within a
dockerfile.
