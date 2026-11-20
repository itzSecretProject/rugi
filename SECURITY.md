# Security policy

Rugi exposes a C ABI that takes pointers, lengths and handles from C and C++
programs, so memory-safety bugs at that boundary are security issues. So are
crashes or hangs that input to a running application can trigger.

## Reporting a vulnerability

Please do not open a public issue. Report it privately through GitHub:
**Security** tab of the repository, then **Report a vulnerability**. Include
the Rugi version, the platform, and a minimal program or input that
reproduces the problem.

You will get a reply within a week. Fixes are released as a patch version
and credited in the changelog unless you ask otherwise.

## Supported versions

Only the latest released version receives security fixes while Rugi is at
0.x.
