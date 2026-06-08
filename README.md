# Codex Proxy

> Routes OpenAI Codex (Responses API) through cheaper providers — DeepSeek, OpenRouter, z.ai, MiniMax, or any OpenAI/Anthropic-compatible API.

Codex Desktop sends requests using the Responses API format, which only OpenAI understands. This proxy translates Responses ↔ Chat Completions (or Anthropic Messages) on the fly, so you can use any model.

## How It Works

```
Codex Desktop → localhost:9090/v1/responses → Codex Proxy → /chat/completions → Your provider
```

## Features

- **API translation** — Responses API ↔ Chat Completions / Anthropic Messages, both directions
- **Streaming** — real-time SSE event translation
- **Tool calling** — full `function_call` / `function_call_output` support
- **Reasoning** — captures DeepSeek `reasoning_content` for multi-turn context
- **Model mapping** — transparently remaps `gpt-5.4` → your chosen model
- **Plugin compatible** — Codex Desktop stays in ChatGPT auth mode so marketplace plugins work

## Quick Start

```bash
pip install -r requirements.txt
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx
```

## Usage

```bash
# Using a preset
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx

# Custom upstream URL
python proxy.py --upstream https://api.deepseek.com --api-key sk-xxx

# Custom port
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx --port 8080
```

### CLI Launcher

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

Settings and API key are stored in `~/.codexproxy/`.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/responses` | Main proxy (Responses API → Chat Completions / Anthropic) |
| `GET` | `/v1/models` | Available models from current model map |
| `GET` | `/health` | Health check |

## Requirements

- Python 3.10+
- Flask, Requests

## License

MIT
