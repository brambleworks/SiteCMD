# SiteCMD updater verifier

This minimal, secretless helper verifies the detached Minisign signature on a
release updater artifact. The release workflow builds it from the protected
candidate commit and checks the signed payload against the updater public key
embedded in that same commit.

It deliberately has one pinned, zero-dependency crate and no access to release
credentials.
