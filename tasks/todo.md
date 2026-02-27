# neurond: Federation Proxy — Task Backlog

Derived from senior tech lead review (2026-02-26).
neurond is the federation proxy that aggregates tools from downstream MCP servers
(e.g., mcpd) and exposes them upstream to cortexd.

Priority scale: **P1** = security/correctness · **P2** = reliability/maintainability · **P3** = code quality · **P4** = testing gaps

---

## P1 — Security & Correctness

### [ ] FIX: Default bind `127.0.0.1` — implement TLS before opening to network

**Category:** Security
**File:** `src/config.rs`
**Status:** Default changed to `127.0.0.1` (2026-02-26)

**Problem:**
neurond is designed to listen on `:8443` for cortexd over mTLS, but there is zero TLS implementation. Opening to the network without TLS allows unauthenticated remote tool invocation.

**Fix:**
1. Add `tls` section to `neurond.toml` (cert_path, key_path, ca_path)
2. Use `axum_server::bind_rustls()` when TLS config is present
3. Validate certs at startup, fail-fast on invalid certs
4. Only then change default bind back to `0.0.0.0`

---

### [ ] IMPLEMENT: Mutual TLS (mTLS) with Certificate Pinning

**Category:** Security

**Description:** Enforce mTLS for the cortexd uplink using statically compiled `rustls`. Implement certificate pinning to reject rogue Root CAs injected into the host OS. Implement automated CSR generation and background rotation of short-lived ephemeral certificates.

---

### [ ] IMPLEMENT: Privilege Dropping

**Category:** Security

**Description:** Integrate the `privdrop` crate (Linux) and `deelevate` crate (Windows). Perform an irreversible privilege drop to the unprivileged `neurond` user immediately after privileged bootstrapping (e.g., binding protected ports, reading certificates).

---

### [ ] IMPLEMENT: Write-Ahead Logging (WAL) for Audit Trail

**Category:** Security / Correctness

**Description:** Integrate `okaywal` to batch and `fsync` JSONL audit logs to disk. Return control only after physical commitment to guarantee durability against OOM kill or power loss.

---

### [ ] FIX: Stdio `stderr` Isolation

**Category:** Correctness

**Problem:** `stderr` from downstream stdio child processes must be strictly redirected to the diagnostic logger. If it bleeds into `stdout` it corrupts the JSON-RPC stream and causes protocol desynchronization.

---

### [ ] FIX: No graceful shutdown — heartbeat orphaned, no deregister

**Category:** Correctness
**File:** `src/main.rs`

**Problem:**
`axum::serve().await` blocks until killed. On SIGTERM:
- `deregister_node()` exists but is never called
- Heartbeat `watch::Sender` dropped without flush
- Downstream connections not drained

**Fix:**
```rust
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.ok();
    // deregister, drop heartbeat, shutdown
});
axum::serve(listener, app)
    .with_graceful_shutdown(...)
    .await?;
```

---

### [ ] FIX: Config validation missing

**Category:** Security / Correctness
**File:** `src/config.rs`

**Problem:** No validation after parsing — `namespace` could be empty or contain dots, `url` in Localhost variant could be malformed, `command` in Stdio variant could be relative (PATH injection), duplicate namespaces silently route to first match.

**Fix:**
```rust
fn validate(&self) -> anyhow::Result<()> {
    let mut seen = HashSet::new();
    for server in &self.federation.servers {
        anyhow::ensure!(!server.namespace.is_empty(), "namespace cannot be empty");
        anyhow::ensure!(!server.namespace.contains('.'), "namespace cannot contain dots");
        anyhow::ensure!(seen.insert(&server.namespace), "duplicate namespace: {}", server.namespace);
        match &server.transport {
            DownstreamTransport::Localhost { url } => {
                url.parse::<url::Url>().context("invalid downstream URL")?;
            }
            DownstreamTransport::Stdio { command, .. } => {
                anyhow::ensure!(command.starts_with('/'), "stdio command must be absolute path: {command}");
            }
        }
    }
    Ok(())
}
```

---

### [ ] IMPLEMENT: Policy-Based Tool Filtering (`list_tools`)

**Category:** Security

**Problem:** `tools/list` returns all aggregated tools from downstreams regardless of policy. The model can see and attempt to call tools it is not authorized to use, causing hallucinations and policy bypass.

**Fix:** Filter the aggregated tool list through the policy engine before returning to upstream. Only expose tools that would pass an `Allow` check. Also needed: log filtered-out tools in the audit trail.

---

### [ ] ENHANCE: Audit Log Schema for Delegated Identity and Duration

**Category:** Security / Auditing

**Description:** Update the JSONL `audit.log` schema to capture `duration_ms` for each tool execution and `obo_identity` when On-Behalf-Of JWT tokens are forwarded from the Agentgateway control plane.

---

### [x] DONE: `node_id` regenerated on every startup — Fixed 2026-02-26

### [x] DONE: Audit log failure must not silently allow mutations — Fixed 2026-02-26

### [x] DONE: Policy engine — deny-wins semantics — Fixed 2026-02-26

### [x] DONE: Extend wildcard matching to full glob patterns — Fixed 2026-02-26

---

## P2 — Reliability & Robustness

### [ ] IMPLEMENT: HTTP Transport Middleware Stack

**Category:** Reliability

**Description:** Layer `tower` middleware onto the `axum` HTTP transport to enforce rate limiting, connection timeouts, request tracing, and payload size limits before JSON-RPC messages reach the execution engine.

---

### [ ] FIX: `route_tool_call` holds RwLock across network I/O

**Category:** Reliability
**File:** `src/federation/manager.rs:98-148`

**Problem:** Read guard held while `client.peer().call_tool().await` executes. A slow downstream (30s timeout) blocks all other calls because `add_downstream` needs a write guard.

**Fix:** Clone client handle before the await, then drop the lock:
```rust
let client = {
    let downstreams = self.downstreams.read().await;
    conn.client.clone()
    // lock dropped here
};
client.peer().call_tool(params).await
```

---

### [ ] FIX: No reconnection logic for failed downstreams

**Category:** Reliability
**Files:** `src/federation/connection.rs`, `src/federation/manager.rs`

**Problem:** `MAX_RETRIES` defined but never used. `mark_restarting()` exists but never called. Dead downstream stays `Healthy` with a dead client.

**Fix:** Spawn a background health check per downstream: periodic `client.peer().ping()`, on failure `mark_restarting()` → retry → `mark_healthy()` or `mark_failed()`.

---

### [ ] FIX: `reqwest::Client` created per-request in registration

**Category:** Performance
**File:** `src/registration/register.rs`

**Fix:** Pass a shared `reqwest::Client` from `main.rs` or use `once_cell::sync::Lazy` to preserve the connection pool across register/deregister calls.

---

### [ ] IMPROVE: Cache aggregated tool list

**Category:** Performance
**File:** `src/federation/manager.rs:80-87`

**Fix:** Cache in `Arc<Vec<Tool>>`, rebuild only when downstreams change. `list_tools` becomes `Arc::clone()` — no allocation per request.

---

### [ ] ADD: One-Line Installation Script

**Category:** Deployment

**Description:** Create `install.sh` that fetches, verifies (checksum + GPG), and installs standalone `neurond` and `mcpd` binaries, then registers and starts the systemd units.

---

### [ ] DOCS: Secure Reverse Proxy Configuration Guide

**Category:** Documentation

**Description:** Document strict configuration for deploying `neurond` behind reverse proxies or Agentgateway. Explicitly warn against `proxy_set_header Host $host` which enables localhost spoofing and auth bypass.

---

## P3 — Code Quality

### [x] DONE: Added `Default` impl for `FederationManager` — Fixed 2026-02-26

### [x] DONE: `sort_by` → `sort_by_key(Reverse)` — Fixed 2026-02-26

### [x] DONE: Format string variables (`{var}` syntax) — Fixed 2026-02-26

### [x] DONE: Removed unused `use tracing;` — Fixed 2026-02-26

### [x] DONE: Suppressed `manual_async_fn` on `ServerHandler` impl — Fixed 2026-02-26

### [x] DONE: Removed unused deps (`dashmap`, `schemars`, `tokio-stream`) — Fixed 2026-02-26

### [x] DONE: Default bind changed `0.0.0.0` → `127.0.0.1` — Fixed 2026-02-26

---

### [ ] CLEANUP: `Effect` should derive `Copy`

**File:** `src/security/policy.rs`

---

### [ ] CLEANUP: `DownstreamConnection` fields should be `pub(crate)` or private

**File:** `src/federation/connection.rs`

**Problem:** All fields are `pub`, bypassing the state machine methods.

---

### [ ] CLEANUP: `gethostname()` should use syscall, not read `/etc/hostname`

**File:** `src/main.rs:99-103`

**Problem:** `/etc/hostname` may be stale if hostname changed at runtime. Use `nix::unistd::gethostname()` or the `hostname` crate instead.

---

## P4 — Testing Gaps

### [ ] ADD: Tests for `connection.rs` state machine

`src/federation/connection.rs` — **0 tests**

Needed: `Configured → Starting → Healthy` transition, `Healthy → Restarting → Failed` transition, `mark_restarting()` increments attempt counter, `mark_failed()` clears tools and client, `is_healthy()` requires both `Healthy` state and `client.is_some()`.

---

### [ ] ADD: Tests for `config.rs` parsing

`src/config.rs` — **0 tests**

Needed: valid `neurond.toml` round-trip, minimal config defaults, missing required fields → error, invalid transport type → error, default bind is `127.0.0.1` and port is `8443`.

---

### [ ] ADD: Integration test for transport layer

`src/federation/transport.rs` — **0 tests**

Needed: spin up in-process MCP server → connect via localhost → list_tools, stdio transport with echo server, invalid URL → meaningful error.

---

### [ ] IMPROVE: `manager.rs` tests with mock downstreams

**File:** `src/federation/manager.rs`

**Problem:** Only empty-state tests exist. Create a mock `ServerHandler` → connect manager → test routing and tool aggregation.

---

### [ ] ADD: End-to-End Integration Tests

Create `tests/integration/` validating real process execution:

1. **`test_stdio_discovery.rs`** — neurond spawns `mcpd` over stdio, handshakes `initialize`, retrieves tools, exposes them prefixed with namespace. Shutdown sends SIGTERM to child, no zombies.
2. **`test_strict_policy_routing.rs`** — strict policy allows `network.ping`, denies `system.*`. Verify allowed calls reach downstream, denied calls return `INVALID_REQUEST` without touching downstream, both logged to `audit.log`.
3. **`test_multi_downstream_isolation.rs`** — 3 downstreams (2 stdio, 1 localhost). `list_tools` aggregates all. Tool call to downstream A does not touch downstream C.
4. **`test_downstream_network_failures.rs`** — downstream is a tarpit. Tool call times out after 30s without holding the `RwLock`. Concurrent calls to other downstreams remain responsive.

---

## Architecture — Future Work

### [ ] DESIGN: Shared types crate (`neurond-protocol` or `mcp-types`)

**Problem:** `RegisterPayload`, namespace conventions, and API paths are defined independently across neurond, mcpd, and cortexd.

**Fix:** Extract a shared crate with registration payloads, heartbeat payloads, status enums, and API path constants.

---

## Linux Deployment

### [ ] ADD: systemd service file (`neurond.service`) with sandboxing

```ini
[Service]
Type=simple
User=neurond
Restart=on-failure
After=network-online.target mcpd.service
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
NoNewPrivileges=yes
ReadWritePaths=/var/log/neurond /etc/neurond
```

---

### [ ] ADD: Native OS Packaging

Use `cargo-deb` and `cargo-rpm` / `rust2rpm`. Include `postinst` / `%post` scripts to provision the `neurond:neurond` system user and restrict `/var/log/neurond` ownership.

---

### [ ] ADD: Append-Only Audit Log (`chattr +a`)

Apply `chattr +a` to `/var/log/neurond/audit.log` post-install. Add `prerotate`/`postrotate` hooks to `logrotate` to temporarily lift the attribute during compression.

---

### [ ] ADD: MAC Profiles (SELinux / AppArmor)

- **RHEL:** SELinux policy permitting `neurond_t` context to send D-Bus messages to `init_t`.
- **Ubuntu:** AppArmor profile confining filesystem access to `/proc/sys`, `/var/log/*` and limiting executable paths for downstream sub-agents.

---

### [ ] ADD: Sample config (`neurond.toml.example`)

Document all fields: bind/port, mcpd downstream (localhost transport), cortexd registration, stdio transport example with absolute binary path.

---

### [ ] ADD: PID file or systemd socket activation support

---

## Windows Deployment

### [ ] ADD: Windows SCM Integration

Integrate the `windows-service` crate using `define_windows_service!` to register the service entry point and correctly delegate Start/Stop SCM signals to the async runtime.

---

### [ ] ADD: Windows Process Tree Management via Job Objects

Assign downstream subprocess handles to Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` to prevent orphaned sub-agents on abrupt SCM termination.

---

### [ ] ADD: Windows Win32 Pipe Management

Use `CreatePipe` for process I/O redirection. Call `SetHandleInformation` to remove the inheritance flag from parent-side pipe ends to prevent handle leaks.

---

### [ ] ADD: NTFS ACLs for Append-Only Auditing

Use `icacls` to grant the service account only `FILE_APPEND_DATA` on the audit log, explicitly denying `FILE_WRITE_DATA` and `DELETE`.
