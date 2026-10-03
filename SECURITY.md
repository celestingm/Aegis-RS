# Security Policy

## Reporting a vulnerability

Please do **not** open a public issue for security problems. Use GitHub's
[private vulnerability reporting](https://github.com/celestingm/Aegis-RS/security/advisories/new)
for this repository.

## Hardening notes

- The `/webhook` endpoint is **disabled** until a real `secret_token` is set in `config.toml`
  (empty or placeholder values such as `change-me` are rejected).
- The API listens on `0.0.0.0`. Put it behind a firewall or reverse proxy with TLS if it is reachable from untrusted networks.
- Aegis-RS runs `docker restart` and `journalctl --vacuum-time`; mounting the Docker socket gives it full control of the host's containers. Only deploy it where that is acceptable.
- Never commit `config.toml`: it contains your token and webhook URLs.
