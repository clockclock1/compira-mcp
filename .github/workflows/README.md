# Keep CI for pull requests / main pushes. Release publishing uses:
#   - build-binaries.yml  (6 OS/arch executables + archives)
#   - docker.yml          (linux amd64/arm64 → GHCR multi-arch)
# Pattern mirrored from https://github.com/clockclock1/Failover-Proxy
