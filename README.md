# Dynatrace Problem Forwarder

A Rust-based service that polls the Dynatrace API for problems and forwards them to configured webhook endpoints. The service tracks forwarded problems in SQLite to prevent duplicates and only forwards new problems or status changes.

## Features

- 🔄 Poll Dynatrace API at configurable intervals
- 🗄️ Track forwarded problems in SQLite to prevent duplicates
- 🔔 Forward only new problems or status changes (OPEN → CLOSED)
- 🔗 Support multiple connector URLs
- 🔁 Retry logic with exponential backoff for failed forwards
- 📊 Comprehensive logging and statistics
- 🛠️ CLI commands for management and testing
- 🚀 Cross-platform binary (macOS, Linux, RHEL)

## Installation

### From Source

```bash
# Clone the repository
git clone <repository-url>
cd dtpf

# Build the release binary
cargo build --release

# The binary will be available at ./target/release/dtpf
```

## Configuration

### 1. Create Configuration File

Copy the example configuration file:

```bash
cp config.yaml.example config.yaml
```

Edit `config.yaml` with your settings:

```yaml
dynatrace:
  base_url: "https://dynatrace.com"
  tenant: "your-tenant-id"
  problem_selector: 'managementZoneIds("000000"),status("open")'

polling:
  interval_seconds: 60
  batch_size: 100

database:
  path: "./data/forwarder.db"

logging:
  level: "info"  # trace, debug, info, warn, error
  format: "pretty"  # json or pretty

connectors:
  - name: "primary-webhook"
    url: "https://your-webhook-endpoint.com/dynatrace"
    method: "POST"
    timeout_seconds: 30
    retry_attempts: 3
    headers:
      Content-Type: "application/json"
```

### 2. Set Environment Variables

Copy the example environment file:

```bash
cp .env.example .env
```

Edit `.env` with your API token:

```bash
# Required: Dynatrace API Token
DYNATRACE_API_TOKEN=dt0c01.XXXXXXXXXXXX.YYYYYYYYYYYY

# Optional: Connector-specific secrets
# WEBHOOK_API_KEY=your-webhook-api-key
```

## Usage

### Run the Forwarder Service

**Foreground (stops when terminal closes):**

```bash
./dtpf run --config ./config.yaml
```

Or using environment variable:

```bash
export CONFIG_PATH=./config.yaml
./dtpf run
```

**Background (runs indefinitely until stopped or server restarts):**

```bash
./dtpf run --nohup --config ./config.yaml
```

This starts the service in the background and creates `dtpf.pid` and `dtpf.log` files in the same directory as your config file.

### Stop Background Service

Stop a background dtpf process:

```bash
./dtpf stop --config ./config.yaml
```

### Clear Cache

Clear all cached problems (forces re-forwarding of all open problems on next poll):

```bash
./dtpf clear-cache --confirm
```

Without `--confirm`, you'll be prompted for confirmation.

### Test Dynatrace Connectivity

Test your Dynatrace API configuration:

```bash
./dtpf test-dynatrace
```

### Test Connectors

Test all configured connectors with a dummy payload:

```bash
./dtpf test-connectors
```

### View Statistics

View database statistics (tracked problems, forward history):

```bash
./dtpf stats
```

Example output:

```
=== Database Statistics ===
Total problems tracked:  150
  Open problems:         27
  Closed problems:       123

Forward history:
  Total forwards:        180
  Successful:            175
  Failed:                5
```

## How It Works

### Polling Loop

1. Every `interval_seconds`, the service fetches problems from the Dynatrace API
2. For each problem:
   - **New problem** (not in database) → Forward to all connectors, insert into database
   - **Status changed** (status differs from database) → Forward update, update database
   - **No change** → Skip (no action)
3. Forward attempts are retried with exponential backoff
4. All forwards are logged in the database for audit

### Deduplication Logic

```
┌─────────────────────────────────────────────┐
│  Fetch problems from Dynatrace API         │
└──────────────┬──────────────────────────────┘
               │
               ▼
      ┌────────────────┐
      │ For each       │
      │ problem        │
      └────┬───────────┘
           │
           ▼
   ┌───────────────────┐
   │ Check database    │
   └───┬───────────────┘
       │
       ├─► Not found ──────────► Forward ──► Insert into DB
       │
       ├─► Status changed ─────► Forward ──► Update DB
       │
       └─► No change ───────────► Skip
```

### Database Schema

The service uses SQLite to track:

- **forwarded_problems**: Problem ID, status, timestamps, forward count
- **forward_history**: Audit log of all forward attempts (success/failure)
- **app_state**: Application state data

## Configuration Reference

### Dynatrace Configuration

```yaml
dynatrace:
  base_url: "https://your-dynatrace-instance.com"
  tenant: "your-tenant-id"
  problem_selector: "status(open)"  # Optional: Dynatrace problem selector
```

**Environment Variables:**
- `DYNATRACE_API_TOKEN` (required): Your Dynatrace API token

### Polling Configuration

```yaml
polling:
  interval_seconds: 60  # Poll every 60 seconds
  batch_size: 100       # Optional: Limit problems per poll
```

### Connector Configuration

```yaml
connectors:
  - name: "webhook-1"
    url: "https://webhook.example.com/endpoint"
    method: "POST"  # POST, PUT, PATCH, GET
    timeout_seconds: 30
    retry_attempts: 3
    verify_ssl: true  # Set to false to disable SSL verification (default: true)
    headers:
      Content-Type: "application/json"
      X-API-Key: "${WEBHOOK_API_KEY}"  # Reference env var
```

**Configuration Options:**

- `verify_ssl`: (Optional, default: `true`) Set to `false` to disable SSL certificate verification. Useful for testing with self-signed certificates or internal systems.

**Environment Variable Substitution:**

Headers can reference environment variables using `${VAR_NAME}` syntax. This is useful for secrets:

```yaml
headers:
  Authorization: "Bearer ${API_TOKEN}"
```

### Logging Configuration

```yaml
logging:
  level: "info"     # trace, debug, info, warn, error
  format: "pretty"  # pretty or json
```

Override with environment variable:

```bash
RUST_LOG=dynatrace_problem_forwarder=debug ./dtpf run
```

## Forwarded Payload

Problems are forwarded as JSON with the full Dynatrace problem structure:

```json
{
  "problemId": "5905480872741084184_1770697620000V2",
  "displayId": "P-260224823",
  "title": "Low disk space",
  "impactLevel": "INFRASTRUCTURE",
  "severityLevel": "RESOURCE_CONTENTION",
  "status": "OPEN",
  "affectedEntities": [...],
  "impactedEntities": [...],
  "rootCauseEntity": {...},
  "managementZones": [...],
  "entityTags": [...],
  "problemFilters": [...],
  "startTime": 1770697800000,
  "endTime": -1
}
```

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running in Development

```bash
cargo run -- run --config ./config.yaml
```

### Cross-Compilation

For Linux (from macOS):

```bash
# Install cross-compilation tool
cargo install cross

# Build for Linux
cross build --release --target x86_64-unknown-linux-gnu
```

## Deployment

### As a Binary

1. Build the release binary:
   ```bash
   cargo build --release
   ```

2. Copy the binary and configuration:
   ```bash
   cp target/release/dtpf /usr/local/bin/
   cp config.yaml /etc/dtpf/
   ```

3. Set up environment variables and run:
   ```bash
   export DYNATRACE_API_TOKEN="your-token"
   export CONFIG_PATH="/etc/dtpf/config.yaml"
   dtpf run
   ```

### Running in Background (Simple Method)

For simple deployments where you want the service to run in the background without systemd:

**1. Start in background:**

```bash
./dtpf run --nohup --config ./config.yaml
```

This will:
- ✅ Run dtpf in the background indefinitely
- ✅ Create a PID file (`dtpf.pid`) to track the process
- ✅ Create a log file (`dtpf.log`) for output
- ❌ **Will NOT** survive server restarts (process stops on reboot)

**2. Stop the background process:**

```bash
./dtpf stop --config ./config.yaml
```

**Note:** The background process will stop when the server restarts. For production deployments that need auto-start on boot and auto-restart on failure, use the systemd method below.

### As a Systemd Service (Production)

For production deployment with auto-start on boot and auto-restart on failure:

**1. Create systemd service file** `/etc/systemd/system/dtpf.service`:

```ini
[Unit]
Description=Dynatrace Problem Forwarder
Documentation=https://github.com/your-repo/dynatrace-problem-forwarder
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=dtpf
Group=dtpf
WorkingDirectory=/opt/dtpf
EnvironmentFile=/etc/dtpf/dtpf.env
ExecStart=/usr/local/bin/dtpf run
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/dtpf/data

[Install]
WantedBy=multi-user.target
```

**2. Create environment file** `/etc/dtpf/dtpf.env`:

```bash
DYNATRACE_API_TOKEN=your-actual-token-here
CONFIG_PATH=/opt/dtpf/config.yaml
RUST_LOG=info
```

**3. Complete installation:**

```bash
# Create user and directories
sudo useradd -r -s /bin/false dtpf
sudo mkdir -p /opt/dtpf/data /etc/dtpf

# Install binary and config
sudo cp target/release/dtpf /usr/local/bin/
sudo cp config.yaml /opt/dtpf/config.yaml

# Copy and configure environment file
sudo cp dtpf.env.example /etc/dtpf/dtpf.env
sudo nano /etc/dtpf/dtpf.env  # Edit with your actual values

# Copy systemd service file
sudo cp dtpf.service.example /etc/systemd/system/dtpf.service

# Set permissions
sudo chown -R dtpf:dtpf /opt/dtpf
sudo chmod 600 /etc/dtpf/dtpf.env  # Protect secrets!

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable dtpf
sudo systemctl start dtpf
```

**4. Manage the service:**

```bash
# Start/Stop/Restart
sudo systemctl start dtpf
sudo systemctl stop dtpf
sudo systemctl restart dtpf

# Check status
sudo systemctl status dtpf

# View logs (real-time)
sudo journalctl -u dtpf -f

# View recent logs
sudo journalctl -u dtpf -n 100 --since today
```

**Key Features:**
- ✅ Auto-starts on server boot
- ✅ Auto-restarts on crash (10 second delay)
- ✅ Survives server restarts
- ✅ Runs in background as systemd service
- ✅ Logs to systemd journal
- ✅ Secure: runs as dedicated non-root user

## Troubleshooting

### API Connection Issues

Test Dynatrace connectivity:

```bash
./dtpf test-dynatrace
```

Common issues:
- Invalid API token: Check `DYNATRACE_API_TOKEN` environment variable
- Network connectivity: Ensure the service can reach your Dynatrace instance
- Incorrect base URL or tenant ID: Verify in `config.yaml`

### Connector Issues

Test connector configuration:

```bash
./dtpf test-connectors
```

Common issues:
- HTTP errors: Check connector URL and authentication
- Timeouts: Increase `timeout_seconds` in connector config
- SSL/TLS errors: Ensure valid certificates

### Database Issues

Check database statistics:

```bash
./dtpf stats
```

If the database is corrupted, you can clear it:

```bash
./dtpf clear-cache --confirm
```

Or manually delete the database file:

```bash
rm -f ./data/forwarder.db
```

### Enable Debug Logging

Edit `config.yaml`:

```yaml
logging:
  level: "debug"
```

Or use environment variable:

```bash
RUST_LOG=dynatrace_problem_forwarder=debug ./dtpf run
```

## License

[Your License Here]

## Contributing

[Your Contributing Guidelines Here]
