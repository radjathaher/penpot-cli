# penpot-cli

Auto-generated Penpot CLI from the backend OpenAPI (RPC) schema. Designed for LLM discovery and direct scripting.

## Install

### Install script (macOS arm64 + Linux x86_64)

```bash
curl -fsSL https://raw.githubusercontent.com/radjathaher/penpot-cli/main/scripts/install.sh | bash
```

### Homebrew (binary, macOS arm64 only)

```bash
brew tap radjathaher/tap
brew install penpot-cli
```

### Build from source

```bash
cargo build --release
./target/release/penpot --help
```

## Auth + base URL

Set a personal access token and your self-host base URL:

```bash
export PENPOT_ACCESS_TOKEN="ppat_..."
export PENPOT_BASE_URL="https://penpot.example.com"
```

Optional override:

```bash
export PENPOT_API_URL="https://penpot.example.com/api/main/methods"
```

## Discovery (LLM-friendly)

```bash
penpot list --json
penpot describe teams get --json
penpot tree --json
```

Human help:

```bash
penpot --help
penpot teams --help
penpot teams get --help
```

## Examples

Get profile:

```bash
penpot profile get --pretty
```

List teams:

```bash
penpot teams get --pretty
```

Create team (example):

```bash
penpot team create --name "My Team" --pretty
```

## Update schema + command tree

```bash
PENPOT_BASE_URL=https://penpot.example.com tools/fetch_openapi.py --out schemas/penpot.openapi.json
tools/gen_command_tree.py --schema schemas/penpot.openapi.json --out schemas/command_tree.json
cargo build
```

## Notes

- Input is sent as JSON body via POST to /api/main/methods/<rpc-method>.
- Use --input '{"json":"body"}' to pass a full request body.
