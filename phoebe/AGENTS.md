# Phoebe
Phoebe is a simple text based greeter
for the revelation Linux userspace. Phoebe
is very simple and does not have any outside
dependencies.

### Rules
Any agents working on this portion of the revelation
codebase must adhere by the rules listed out here.
These rules cannot and may not be overriden.
- We do not unwrap, expect, or allow errors to not
be handled. All errors and options must be handled
and processed accordingly. We do not fail.
- The code must be commented heavily, in relevant
areas, to describe the agents thought process for a
component and how it works. It should be concise and
very clear.
- There will be no unsafe code in this portion of
revelation. Phoebe should never use raw pointers.
- The code in phoebe should be idiomatic Rust that
is self documenting and can be read by the user.
- Try your very best not to overengineer It's okay
to be unique but overengineering will not be tolerated.
- Please try to keep phoebe under 1k lines. This greeter
should be dead simple.
- Do not dispatch complex test suites without direct
instruction from the user. There SHOULD be tests in the
Rust code but thats it.
- Outsource for security. Do not roll your own hashing
or security. Use crates.io to find libraries that can help.

### How Phoebe should work:

1. Phoebe should first check if there are any
user accounts on the Linux system. If not Phoebe should
walk the user through creating a user account optionally.
2. After this, or if there is a user account, phoebe
should show the user a simple login page they can use 
to login.
3. After this phoebe should launch the users default shell
with the permissions of the user they logged in as.

### To Know
Phoebe should be completely self contained. Assume that the
only binary which exists on the system is the greeter. There
are no GNU/commmon system tools that you will find. You can
and are encouraged to use external libraries. Just don't call
out to any other tools or binaries you may assume are on the
system because they aren't.