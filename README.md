# Codex Proxy

> Routes OpenAI Codex (Responses API) through cheaper providers — DeepSeek, OpenRouter, z.ai, MiniMax, or any OpenAI/Anthropic-compatible API.

**Why?** Codex Desktop sends requests using the Responses API format, which only OpenAI understands. This proxy translates Responses ↔ Chat Completions (or Anthropic Messages) on the fly, letting you use any model you want.

## How It Works

```
Codex Desktop → localhost:9090/v1/responses → Codex Proxy → upstream/chat/completions → Your provider
```

## Features

- **API translation** — Responses API ↔ Chat Completions / Anthropic Messages, both directions
- **Streaming** — real-time SSE event translation with zero buffering
- **Tool calling** — full `function_call` / `function_call_output` support
- **Reasoning** — captures DeepSeek `reasoning_content` and re-injects it for multi-turn context
- **Model mapping** — transparently remaps `gpt-5.4` → your chosen model
- **macOS integration** — writes Codex config, manages env vars for Codex Desktop app
- **Plugin compatible** — Codex Desktop stays in ChatGPT auth mode so marketplace plugins keep working

## Quick Start

```bash
pip install -r requirements.txt
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx
```

Then configure Codex to use the proxy (see [Configuration](#configuration) below).

## Usage

### GUI (pywebview)

```bash
python app.py
```

Opens a native macOS window. Enter your API key, pick a provider, hit Start.

### GUI (Tkinter fallback)

```bash
python app_tkinter.py
```

Same features, works without pywebview dependency.

### CLI (headless)

```bash
# Using a preset
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx

# Custom upstream URL
python proxy.py --upstream https://api.deepseek.com --api-key sk-xxx

# Custom port
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx --port 8080
```

### CLI Launcher (for Codex CLI)

```bash
./launch-codex.sh
```

Reads API key from `~/.codexproxy/.api_key` and launches Codex CLI with `--profile proxy`.

## Supported Providers

| Provider | Preset | API Type | Upstream |
|----------|--------|----------|----------|
| **DeepSeek** | `"DeepSeek V4 Pro"` | OpenAI | `api.deepseek.com` |
| **OpenRouter** | `"OpenRouter"` | OpenAI | `openrouter.ai/api/v1` |
| **MiniMax** | `"MiniMax"` | OpenAI | `api.minimax.chat/v1` |
| **OpenCode** | `"OpenCode"` | OpenAI | `opencode.ai/ru/go` |
| **z.ai** | `"z.ai"` | Anthropic | `api.z.ai/api/anthropic` |
| **LM Studio** | `"LM Studio"` | OpenAI | `localhost:1234/v1` |
| **Ollama** | `"Ollama"` | OpenAI | `localhost:11434/v1` |

For any other OpenAI-compatible endpoint, use `--upstream <url>` without a preset.

## Configuration

### Codex Config

The proxy needs to tell Codex where to send requests. Two options:

**Option A — Automatic install** (from GUI: Settings → Install Codex Config):

Creates a proxy provider section in `~/.codex/config.toml` with a timestamped backup of your existing config.

**Option B — Manual:**

Copy `config.toml.example` to `~/.codex/config.toml`:

```toml
model = "gpt-5.4"
model_provider = "codex-proxy"
sandbox_mode = "danger-full-access"

[model_providers.codex-proxy]
name = "Codex Proxy"
base_url = "http://localhost:9090/v1"
env_key = "CODEX_PROXY_API_KEY"
wire_api = "responses"
```

### API Key

Priority order:

1. `Authorization` header in the request
2. `--api-key` CLI argument
3. `CODEX_PROXY_API_KEY` environment variable
4. Empty (requests pass through without auth)

### Environment Variables

See `.env.example` — the only variable is `CODEX_PROXY_API_KEY`.

### Settings

Runtime settings are stored in `~/.codexproxy/settings.json`. The proxy creates this directory automatically. API key is stored separately in `~/.codexproxy/.api_key` (chmod 600).

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/responses` | Main proxy endpoint (Responses API → Chat Completions / Anthropic) |
| `GET` | `/v1/models` | Returns available model names from current model map |
| `GET` | `/health` | Health check |

Both `/v1/responses` and `/responses` work interchangeably.

## Project Structure

```
proxy.py                      # Core proxy server (Flask)
app.py                        # pywebview GUI
app_tkinter.py                # Tkinter GUI fallback
config.py                     # Settings, API key, Codex config management
codex-proxy-redesign.html     # UI for pywebview app
launch-codex.sh               # Codex CLI launcher script
config.toml.example           # Example Codex config
build.sh                      # Build native macOS .app
CodexProxyMenuBar.swift       # Swift menu bar app source
app_native.py                 # Python backend for Swift app
requirements.txt              # Python dependencies
```

## Build Native macOS App

```bash
./build.sh
# Output: build/CodexProxy.app
cp -r build/CodexProxy.app /Applications/
```

See [BUILD.md](BUILD.md) for manual build steps.

## Requirements

- Python 3.10+
- Flask, Requests (`pip install -r requirements.txt`)
- pywebview (optional, for GUI — `pip install pywebview`)

## License

MIT
