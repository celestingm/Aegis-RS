# 🛡️ Aegis-RS (Autonomous Remediation System)

> **A rust-based, modular, and autonomous self-healing orchestrator for your infrastructure.**

Aegis-RS monitors your system (Disk, Docker Services) and **automatically repairs faults** (restarting containers, cleaning logs) without human intervention. Use it as a standalone binary or deploy it alongside your stack.

---

## 🚀 Key Features

*   **🔍 Auto-Discovery**: Automatically monitors any container with the label `aegis.monitor=true`. No config needed!
*   **⚡ Blazing Fast**: Written in Rust, using Tokio and Axum. Low footprint.
*   **🛠️ Self-Healing**: Detects DOWN services and restarts them. Detects disk saturation and cleans logs.
*   **🔔 Notifications**: Sends real-time alerts to **Discord**, **Slack**, or any Webhook.
*   **📈 Visualization**: Built-in `/graph` endpoint renders SVG history of system stats.
*   **🛡️ Secure**: Webhook endpoint protected by Bearer Token.

---

## 📦 Installation & Usage

### 1. The "Magic" Way (Docker Labels)

Simply add the label to your `docker-compose.yml`:

```yaml
services:
  my-app:
    image: nginx:latest
    labels:
      - "aegis.monitor=true"
```

Then run Aegis-RS:

```bash
cargo run --release
```

### 2. Configuration (Optional)

Create a `config.toml` file to customize behavior:

```toml
# API Port (default 3000)
api_port = 3001

# Secret token for external webhooks
secret_token = "my-super-secret"

# Disk usage threshold (default 90%)
disk_threshold = 95

# Webhook for Notifications (Discord/Slack/etc)
webhook_url = "https://discord.com/api/webhooks/..."

# (Optional) Hardcoded list of services instead of Auto-Discovery
# docker_services = ["nginx", "redis"] 
```

Run with specific config:

```bash
./aegis-rs --config /path/to/config.toml
```

---

## 📡 API Endpoints

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/metrics` | Prometheus-compatible system metrics. |
| `GET` | `/graph` | SVG Graph of disk usage (last 60m). |
| `POST` | `/webhook` | Trigger manual remediation (Requires Bearer Token). |

---

## 🏗️ Project Structure

The project is structured for modularity and maintainability:

```
src/
├── core/           # Business Logic
│   ├── monitor.rs      # System & Docker Monitoring
│   ├── remediation.rs  # Healing Logic (Restart, Clean)
│   ├── notification.rs # Webhook Alerting
│   └── config.rs       # TOML/Env Parsing
├── web/            # HTTP Interface
│   └── api.rs          # Axum Server & Routes
└── main.rs         # Entrypoint & Orchestration Loop
```

---

## 📜 License

MIT License. Built with ❤️ in Rust.
