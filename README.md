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

### 1. Download Binary

Go to the **Actions** tab on GitHub, click the latest workflow run on `main`, and download the `aegis-rs-linux` artifact.


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
├── domain/           # Enterprise Business Rules (Entities, Ports)
│   ├── mod.rs
│   ├── entities.rs   # Structs: HealthStatus, Alert, Config...
│   └── ports.rs      # Traits: MonitorPort, RemediationPort...
├── application/      # Application Business Rules (Use Cases)
│   ├── mod.rs
│   └── orchestrator.rs # The "Brain": Orchestrates the loop
├── infrastructure/   # Frameworks & Drivers (Adapters)
│   ├── mod.rs
│   ├── docker_monitor.rs
│   ├── disk_monitor.rs
│   ├── config_loader.rs
│   └── notification_adapter.rs
├── presentation/     # Interface Adapters (Controllers)
│   ├── mod.rs
│   └── api/          # Axum Handlers
└── main.rs           # Composition Root & wiring
```

## 🛠️ Development & Testing

We use a `Makefile` to simplify common tasks.

*   **Run everything (Format, Lint, Test, Build)**:
    ```bash
    make ci
    ```
*   **Run only tests**:
    ```bash
    make test
    # or
    cargo test
    ```
*   **Run the app locally**:
    ```bash
    make run
    # or
    cargo run --release
    ```

---

## 📜 License

MIT License. Built with ❤️ in Rust.
