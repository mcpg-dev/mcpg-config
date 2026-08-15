# mcpg-config

The configuration toolchain for the [MCP Gateway](https://github.com/mcpg-dev/mcpg).
It speaks the gateway's real configuration schema — same parser, same
`deny_unknown_fields` strictness — so a file this tool accepts is a file the
gateway boots.

```
mcpg-config init [shape]     Generate a starter config from a curated
                             deployment shape (dev single-node, production
                             with Redis or NATS, air-gapped, multi-tenant)
mcpg-config check <file>     Validate a config against the gateway schema,
                             with the gateway's own error messages
mcpg-config explain <file>   Show the effective wiring a config resolves to
mcpg-config secrets <file>   Scan for inline secrets that belong in cred://
                             references or environment substitutions
mcpg-config doc              Print the full configuration reference
mcpg-config schema           Emit the configuration JSON schema
```

Installed next to the `mcpg` binary it is also reachable as `mcpg config
<subcommand>` — the gateway CLI delegates to this tool.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/mcpg-dev/source-code/main/install.sh | sh -s -- --bin mcpg-config
```

or from source, by git reference:

```sh
cargo install mcpg-config --git https://github.com/mcpg-dev/mcpg-config --tag <release-tag>
```

Release tags, signed artifacts, and checksums are on this repository's
Releases page.

## About this repository

This repository is read-only: development happens upstream, and each release
is published here as a tagged snapshot. Issues are welcome.
