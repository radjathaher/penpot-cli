---
title: penpot-cli
status: closed
priority: 1
issue-type: task
created-at: "\"\\\"2026-01-27T21:46:48.835110+07:00\\\"\""
closed-at: "2026-01-27T21:57:03.243144+07:00"
close-reason: "Implemented Penpot RPC CLI, schema tooling, install script, and release packaging. Validation: cargo build --release."
---

Rust CLI for Penpot RPC API (self-host placeholder). Files: Cargo.toml, src/main.rs, src/command_tree.rs, src/http.rs, schemas/penpot.openapi.json, schemas/command_tree.json, tools/fetch_openapi.py, tools/gen_command_tree.py, scripts/install.sh, dist/*, README.md, Formula/penpot-cli.rb. Accept: env PENPOT_BASE_URL + PENPOT_ACCESS_TOKEN; list/describe/tree; resource/op invocation; build arm64 tar.gz; release upload.
