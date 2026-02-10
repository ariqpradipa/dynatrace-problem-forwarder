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

```bash
./dtpf run --config ./config.yaml
```

Or using environment variable:

```bash
export CONFIG_PATH=./config.yaml
./dtpf run
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

### As a Systemd Service

Create `/etc/systemd/system/dtpf.service`:

```ini
[Unit]
Description=Dynatrace Problem Forwarder
After=network.target

[Service]
Type=simple
User=dynatrace
Environment="DYNATRACE_API_TOKEN=your-token"
Environment="CONFIG_PATH=/etc/dtpf/config.yaml"
ExecStart=/usr/local/bin/dtpf run
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable dtpf
sudo systemctl start dtpf
sudo systemctl status dtpf
```

View logs:

```bash
sudo journalctl -u dtpf -f
```

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
