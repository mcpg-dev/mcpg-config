## Operator workflow

The four config-tooling binaries cover the operator's full loop — pick a starting point, drill into a field, validate, boot:

```bash
$ mcpg config init                                    # pick a deployment template, write config.yaml
$ mcpg config explain governance.audit.on_failure    # describe a field by dotted path
$ mcpg config check config.yaml                       # pre-flight validate (multi-file too)
$ MCPG_CONFIG=config.yaml mcpg                        # boot
```

Multi-file layering is supported — files later on the command line override earlier ones, and `MCPG_*` env vars apply last:

```bash
$ MCPG_CONFIG=base.yaml:production-overrides.yaml mcpg
```

For in-place config rotation (`kill -HUP`) vs restart-required fields, see the [config sources & hot-reload guide](https://mcpg.dev/docs/gateway/config-sources).

For IDE autocomplete, point your YAML language server at the committed schema:

```yaml
# yaml-language-server: $schema=../../examples/deployments/config.schema.json
gateway:
  server:
    bind_address: "127.0.0.1:8787"
    # ↑ IDE autocompletes here, with `///` doc-comment as hover.
```

---

## Top-level keys (Layout D'')

Layout D'' collapsed the pre-D'' flat root into seven typed top-level keys. The migration map for any pre-D'' YAML or env-var examples you might still be reading:

| Pre-D'' key | Layout D'' key |
|---|---|
| `auth:` | `governance.access:` |
| `policy:` | `governance.policy:` |
| `audit:` | `governance.audit:` |
| `approvals:` | `governance.approvals:` |
| `server:` | `gateway.server:` |
| `admin:` | `gateway.admin:` |
| `control_plane:` | `gateway.control_plane:` |
| `content_storage:` | `storage:` |
| `mcp.tools[]` (etc.) | `mcp.capabilities.tools[]` (etc.) |
| `plugins.entries[]` | `plugins[]` (flat array, no wrapper) |
| `plugins.kv` / `caches` / `secrets` / `configs` / `transports` / `policy` / `capability_grants` / `trust` / `credentials` | DELETED — point-of-use slots + per-entry configuration replace these |
| `plugins.health_probe` | `observability.plugin_health_probe` |
| `plugins.registry` | `gateway.plugin_registry` |
| `plugins.config_overlay` | `gateway.config_overlay` |
| `plugins.response_cache` | `storage.response_cache` |
| `plugins.enabled` / `plugins.plugin_dir` | DELETED |
| `BackendImpl.type:` (in a binding's `backend:` block) | `BackendImpl.kind:` |

The ten D'' top-level keys are: `mcp:`, `governance:`, `gateway:`, `observability:`, `feature_flags:`, `debug:`, `schema_registry:`, `storage:`, `cluster:`, `plugins:` (plus `MCPG_CONFIG`-only `config_source:`). `governance:` and `gateway:` are umbrellas — their children correspond one-to-one to former root peers.

`MCPG_*` env vars track the new shape with `__` as the dotted-path separator. The corresponding env-var prefix migrations:

| Pre-D'' env var | Layout D'' env var |
|---|---|
| `MCPG_AUTH__*` | `MCPG_GOVERNANCE__ACCESS__*` |
| `MCPG_POLICY__*` | `MCPG_GOVERNANCE__POLICY__*` |
| `MCPG_AUDIT__*` | `MCPG_GOVERNANCE__AUDIT__*` |
| `MCPG_APPROVALS__*` | `MCPG_GOVERNANCE__APPROVALS__*` |
| `MCPG_SERVER__*` | `MCPG_GATEWAY__SERVER__*` |
| `MCPG_ADMIN__*` | `MCPG_GATEWAY__ADMIN__*` |
| `MCPG_CONTROL_PLANE__*` | `MCPG_GATEWAY__CONTROL_PLANE__*` |
| `MCPG_CONTENT_STORAGE__*` | `MCPG_STORAGE__*` |
| `MCPG_MCP__TOOLS__*` (etc.) | `MCPG_MCP__CAPABILITIES__TOOLS__*` (etc.) |
| `MCPG_PLUGINS__HEALTH_PROBE__*` | `MCPG_OBSERVABILITY__PLUGIN_HEALTH_PROBE__*` |
| `MCPG_PLUGINS__REGISTRY__*` | `MCPG_GATEWAY__PLUGIN_REGISTRY__*` |
| `MCPG_PLUGINS__CONFIG_OVERLAY__*` | `MCPG_GATEWAY__CONFIG_OVERLAY__*` |
| `MCPG_PLUGINS__RESPONSE_CACHE__*` | `MCPG_STORAGE__RESPONSE_CACHE__*` |

---

## Quick start (minimal viable)

Goal: gateway answers `/health`, `tools/list`, and one `tools/call` against your binding. No auth, no cluster, no compliance plumbing.

**Template:** `dev-single-node` (`mcpg config init --template dev-single-node`). Six fields are load-bearing — everything else takes a sensible default.

| Field | Why it matters | Default | Reference |
|---|---|---|---|
| `gateway.server.bind_address` | The TCP listener. `127.0.0.1:8787` for dev; `0.0.0.0:8787` once you trust auth. | `"127.0.0.1:8787"` | [`ServerConfig`](#serverconfig) |
| `gateway.server.allowed_origins` | CORS allowlist for browser clients. Empty disables browser-cross-origin. | `[]` | [`ServerConfig`](#serverconfig) |
| `mcp.capabilities.tools[]` (and `prompts[]`, `resources[]`, `resource_templates[]`) | The tools / prompts / resources this gateway exposes. At least one entry to be useful. | `[]` | [`McpConfig`](#mcpconfig), [`BackendConfig`](#backendconfig) |
| `governance.access` | Inbound identity. Empty = anonymous (loopback only). | `{}` | [`AccessConfig`](#accessconfig) |
| `observability.logs.sinks` | Where logs go. Default stderr-JSON is fine for dev. | one stderr JSON sink | [`LogsConfig`](#logsconfig) |
| `governance.audit` | Compliance audit. On by default with the built-in local-file sink — drop directory into a tmpfs / scratch mount if your dev disk is read-only. | enabled, file sink | [`AuditConfig`](#auditconfig) |

**Boot it:**

```bash
$ mcpg config init --template dev-single-node --output config.yaml
$ mcpg config check config.yaml
$ MCPG_CONFIG=config.yaml mcpg
```

---

## Production hardening

Goal: external traffic, OIDC, audit you can ship to compliance, multi-replica behind an LB. Pick this once dev clicks.

**Templates:** `production-single-redis` (single instance), `production-redis-cluster` (multi-replica), `production-nats-cluster` (NATS variant) — all available via `mcpg config init --template <name>`.

| Block | What it gates | Reference |
|---|---|---|
| `gateway.server.tls` | Listener TLS — drop if your LB terminates TLS instead. | [`TlsConfig`](#tlsconfig) |
| `gateway.server.allowed_origins` | Browser CORS allowlist. Wildcards are rejected. | [`ServerConfig`](#serverconfig) |
| `gateway.server.max_sessions_per_tenant` | Per-tenant session quota. 0 = unlimited; tighten for SaaS deploys. | [`ServerConfig`](#serverconfig) |
| `cluster` | Coordinator (KV + pub/sub). `single_node` for single-replica, `redis` / `nats` for multi-replica. | [`ClusterConfig`](#clusterconfig) |
| `governance.access.oidc_oauth` | Inbound OIDC — verifies Bearer tokens against your IdP's JWKS. | [`OidcOAuthConfig`](#oidcoauthconfig) |
| `governance.access.jwks` | Static JWKS variant for air-gapped deploys (no IdP discovery call). | [`JwksConfig`](#jwksconfig) |
| `governance.policy` | Pre-dispatch tool gate. `default_minimum_trust` + per-tool overrides + CEL `allow_if`. | [`PolicyConfig`](#policyconfig) |
| `governance.audit` | Compliance audit fan-out. `required: true` refuses to boot without a serving sink. | [`AuditConfig`](#auditconfig) |
| `governance.approvals` | Human-in-the-loop approvals — signing key + callback URL + grace window. | [`ApprovalsConfig`](#approvalsconfig) |
| `plugins[]` | Tool-gate / transform / identity / cluster / catalog plugins. Rate limiting, IP allowlist, circuit breakers all live here. Each entry carries its own `signature.trusted_keys:` + `granted_capabilities:` (per-entry, not a wiring block). | [`PluginEntryConfig`](#pluginentryconfig) |
| `observability.metrics` / `traces` | Prometheus scrape endpoint + OTLP traces to your collector. | [`MetricsConfig`](#metricsconfig), [`TracesConfig`](#tracesconfig) |

**Cluster pub/sub inheritance.** Capability `store:`/`bus:` overrides default to `kind: cluster` (the cluster backend's primitive) when omitted, so when `cluster.kind` is `redis` or `nats`, `mcp.configurations.delivery.bus` and `mcp.configurations.cancellation.bus` automatically use the cluster's pub/sub. Server-initiated messages (cancellations, sampling responses, elicitations) reach the right replica without operator config. Override these only when you explicitly want single-replica behaviour despite a cluster (e.g. `bus: { kind: memory }`).

---

## Advanced / experimental

Goal: pipeline tools, server-initiated suspensions, per-plugin observability carve-outs, control-plane attachment. Ignore until production basics are solid.

| Feature | Block | Reference |
|---|---|---|
| Pipeline bindings — multi-step tools that chain HTTP / SQL / Command / Transform / CEL gate steps. | `mcp.capabilities.tools[].backend: { kind: pipeline, steps: [...] }` | [`PipelineBackendConfig`](#pipelinebackendconfig), [`PipelineStepConfig`](#pipelinestepconfig) |
| Suspending pipeline steps — `elicitation`, `sampling`, `roots_list`. The pipeline pauses, the gateway sends a server-initiated request, the step resumes when the client responds. | `mcp.capabilities.tools[].backend.steps[].kind: elicitation \| sampling \| roots_list` | [`PipelineElicitationStepConfig`](#pipelineelicitationstepconfig), [`PipelineSamplingStepConfig`](#pipelinesamplingstepconfig), [`PipelineRootsListStepConfig`](#pipelinerootsliststepconfig) |
| Approval gates — block a tool call until a human approves it via callback URL. | `governance.approvals` + plugin entry for the approvals provider | [`ApprovalsConfig`](#approvalsconfig) |
| MCP App URL — link a resource to a rich UI for client-side rendering. | `mcp.capabilities.resources[].mcp_app_url` / `mcp.capabilities.resource_templates[].mcp_app_url` (CEL-templatable) | [`BackendConfig`](#backendconfig) |
| Resource subscription with custom watch strategy — push-based change notification with `notifications/resources/updated`. | `mcp.capabilities.resources[].watch` / `mcp.capabilities.resource_templates[].watch` | [`ResourceWatchConfig`](#resourcewatchconfig) |
| Per-plugin observability override — silence a noisy plugin or boost its verbosity in isolation, optionally redirect its events to a separate sink set. | `plugins[].observability` | [`PluginObservabilityToggle`](#pluginobservabilitytoggle), [`SignalToggle`](#signaltoggle), [`SinkMode`](#sinkmode) |
| Control-plane attachment — gateway registers with a CP at boot, opens an agent Channel, ships per-tool-call samples. | `gateway.control_plane` + `cp-attached` Cargo feature | [`ControlPlaneAttachConfig`](#controlplaneattachconfig) |
| Notification filter — server-side filtering of `tools/list_changed` etc. before broadcast. | `mcp.capabilities.resources[].watch.notification_filter` / `mcp.capabilities.resource_templates[].watch.notification_filter` | [`NotificationFilterConfig`](#notificationfilterconfig) |
| Plugin config overlay — operator-staged dynamic config delivered via config-provider plugins (consul / k8s ConfigMap / etc.). | `gateway.config_overlay` | [`ConfigOverlayConfig`](#configoverlayconfig) |

---

## Multi-tenant deployments

MCPG has no top-level `tenants:` block by design. Per-tenant differentiation composes from three orthogonal primitives that already exist:

1. **Tenant identity.** Comes off the verified principal — `identity.subject_id`, `identity.attributes.<claim>`, `identity.roles[]`, `identity.groups[]`. Whichever OIDC claim represents your tenant (commonly `tid`, `org_id`, or a custom claim) ends up under `identity.attributes.<claim>` once the inbound JWT verifier resolves it. No config knob is needed for the tenant ID itself.

2. **Per-tenant binding allowlist.** Goes through `governance.policy.tool_access.rules[].cel_allow_if`. Each binding gets a CEL predicate that references the principal's tenant claim:

   ```yaml
   governance:
     policy:
       tool_access:
         default_minimum_trust: verified
         rules:
           - tool_name: "acme.*"
             cel_allow_if: 'identity.attributes.tenant == "acme" || "platform" in identity.groups'
           - tool_name: "partner.*"
             cel_allow_if: 'identity.attributes.tenant == "partner"'
           # shared.* falls through to default_minimum_trust (any verified caller).
   ```

   The CEL predicate runs pre-dispatch alongside trust-floor checks, so a deny is observable through the same audit shape (`mcpg.policy.tool_call.denied`) as any other policy denial.

3. **Per-tenant rate limit + quota.** Lives in the rate-limit plugin (rate limiting is plugin-only). The plugin's config is keyed by tenant identity from the same `identity` surface. Example shape (rate-limit plugin's config; the actual fields depend on the plugin you load):

   ```yaml
   plugins:
     - id: dev.mcpg.builtin.rate_limit
       config:
         default:
           tools_per_minute: 1000
         by_tenant:
           acme: { tools_per_minute: 10000, burst: 200 }
           partner: { tools_per_minute: 100, burst: 10 }
         tenant_key: 'identity.attributes.tenant'
   ```

   Per-binding rate-limit references go through the backend's own per-use slot (point-of-use wiring) — same shape as any other plugin reference.

The pre-existing `gateway.server.max_sessions_per_tenant` knob is the one *gateway-resident* tenant-aware quota; it's enforced inside the session store (per-tenant cap on concurrent sessions) and works regardless of which CEL gate let the request through.

A first-class `tenants:` block that desugars into the above is a possible future addition. Until the recipes turn out painful in operator hands, the existing primitives stay the source of truth — adding a parallel mechanism would split tenant config across two places.

---

## Templating and secret resolution

MCPG uses a single CEL-based expression syntax everywhere — `${...}` outer markers wrap a CEL expression. There's exactly one form for environment variables, identity, arguments, OAuth tokens, and credential lookups; no parallel layers.

```yaml
mcp:
  capabilities:
    tools:
      - name: github.user.repos.list
        description: Fetch a user's repositories from GitHub.
        backend:
          kind: http
          url: "https://api.github.com/users/${arguments.username}/repos"
          method: get
          headers:
            Authorization: "Bearer ${env.GITHUB_TOKEN}"
            X-Trace-Id: "${context.principal_id}-${arguments.username}"

gateway:
  plugin_registry:
    auth:
      username: "${env.GHCR_USERNAME}"
      password: "${env.GHCR_TOKEN}"

plugins:
  # Outbound OAuth 2.0 (RFC 6749 client_credentials) lives behind
  # the `dev.mcpg.credential.oauth-client-credentials` plugin.
  # Bindings reference issued tokens via the standard `cred://`
  # URI scheme — see the table below.
  - id: dev.mcpg.credential.oauth-client-credentials
    config:
      providers:
        analytics:
          token_url: "https://auth.example.com/oauth/token"
          client_id: "mcpg-prod"
          client_secret: "${env.ANALYTICS_CLIENT_SECRET}"
          scopes: ["read:events"]
```

**Available roots:**

| Root | Resolved at | Notes |
|---|---|---|
| `env.<NAME>` | Config-load (once) | Process environment. Errors if unset. |
| `arguments.<key>` | Per request | Tool-call arguments. |
| `identity.<field>` | Per request | `subject_id`, `attributes.<key>`, `roles[N]`, `groups[N]`, … |
| `cred://<plugin_id>/<target>[#part]` | Per request | Credential plugin lookup. Covers outbound OAuth tokens (`cred://dev.mcpg.credential.oauth-client-credentials/<provider>`), Vault dynamic DB creds (`cred://vault-dynamic-db/orders#username`), and any other registered `credential_issuer` plugin. |
| `context.<field>` | Per request | Transport, principal, trust level, etc. |
| `tool_name` | Per request | Current tool's MCP name. |
| `steps.<id>.output` | Pipeline only | Previous step's result. |

`env.X` is resolved once at config-load — restarts pick up new values. Everything else is per-request, so a token rotation reaches in-flight calls on the next dispatch without a reload.

---

## Reference

What follows is the full alphabetical reference of every type reachable from `AppConfig`, generated from `///` rustdoc + `#[serde(...)]` annotations. Use `mcpg config explain <field>` to drill into a single field on the command line.

