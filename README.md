# Codex Proxy

Route OpenAI Codex Desktop through cheaper providers. Translates the Responses API to Chat Completions or Anthropic Messages on the fly, so you can use DeepSeek, OpenRouter, z.ai, MiniMax, LM Studio, Ollama — or any compatible API.

**Saves 30–50× on API costs** compared to OpenAI pricing.

## How It Works

```
Codex Desktop ──▶ localhost:9090/v1/responses ──▶ Codex Proxy ──▶ Your provider
```

Codex Desktop only speaks the Responses API format. This proxy translates it transparently — no Codex modifications needed.

## Features

- **API translation** — Responses API ↔ Chat Completions / Anthropic Messages
- **Streaming** — real-time SSE event translation
- **Tool calling** — full `function_call` / `function_call_output` support
- **Reasoning capture** — stores DeepSeek `reasoning_content` for multi-turn context
- **Model mapping** — remaps `gpt-5.4` → your chosen model transparently
- **Plugin compatible** — Codex stays in ChatGPT auth mode so marketplace plugins work

## Install

```bash
pip install -r requirements.txt
```

Requires Python 3.10+. Only two core deps: `flask` and `requests`.

## Three Ways to Use

### 1. Desktop GUI

Pick a provider, paste your API key, hit Start.

```bash
# pywebview — HTML/CSS UI with brutalist design
pip install pywebview
python app.py

# Tkinter — native macOS widgets, no extra deps
python app_tkinter.py
```

### 2. Native macOS App

Menu bar app with a full Settings window — provider selection, API key validation, model picker, logs. Built with Swift + Python backend.

```bash
bash build.sh          # build CodexProxy.app
open build/CodexProxy.app
```

Requires macOS 12+ and Xcode Command Line Tools (`xcode-select --install`).

### 3. Command Line

```bash
# Using a preset
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx

# Custom upstream URL
python proxy.py --upstream https://api.deepseek.com --api-key sk-xxx

# Custom port
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx --port 8080
```

## Supported Providers

| Provider | Preset | API Type |
|----------|--------|----------|
| **DeepSeek** | `"DeepSeek V4 Pro"` | OpenAI |
| **OpenRouter** | `"OpenRouter"` | OpenAI |
| **MiniMax** | `"MiniMax"` | OpenAI |
| **OpenCode** | `"OpenCode"` | OpenAI |
| **z.ai** | `"z.ai"` | Anthropic |
| **LM Studio** | `"LM Studio"` | OpenAI (local) |
| **Ollama** | `"Ollama"` | OpenAI (local) |

For any other OpenAI-compatible endpoint, use `--upstream <url>`.

## Codex Configuration

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

The GUI and native app install this config automatically via the "Install Config" button.

## API Key

Priority order:

1. `Authorization` header in the request
2. `--api-key` CLI argument
3. `CODEX_PROXY_API_KEY` environment variable
4. Empty (no auth)

Settings and API keys are stored in `~/.codexproxy/`.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/v1/responses` | Main proxy (Responses API → Chat Completions / Anthropic) |
| `GET` | `/v1/models` | Available models from current model map |
| `GET` | `/health` | Health check |

## Project Structure

```
proxy.py                    # Core proxy server
config.py                   # Settings & Codex config management
app.py                      # pywebview GUI
app_tkinter.py              # Tkinter GUI
codex-proxy-redesign.html   # HTML UI for pywebview
app_native.py               # Python backend for Swift app
CodexProxyApp/              # Swift menu bar app source
build.sh                    # Build script for macOS app
```

## License

MIT
