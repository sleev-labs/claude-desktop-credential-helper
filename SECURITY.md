# Security policy

## Reporting a vulnerability

Please do not open a public issue for security problems.

Report them privately through GitHub's vulnerability reporting form:
https://github.com/sleev-labs/claude-desktop-credential-helper/security/advisories/new

We will acknowledge the report as soon as possible and keep you informed
while a fix is prepared and released.

## Scope

This program reads, refreshes, and persists the Claude OAuth token stored by
the `claude` CLI, and prints it for Claude Desktop. Anything that could
expose that token to another process or user, cause the wrong token to be
printed, or tamper with released binaries is in scope.

## Verifying releases

Every released archive carries a build provenance attestation linking it to
the public GitHub Actions run that built it:

```sh
gh attestation verify <archive> --repo sleev-labs/claude-desktop-credential-helper
```
