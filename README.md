# Codex Proxy for macOS

macOS menu bar app that proxies OpenAI Codex to cheaper models (DeepSeek, GLM, OpenRouter).

**Saves 30-50x** on API costs by routing Codex requests through DeepSeek V4 Pro (~$0.3-0.6/M tokens) instead of GPT-5.5 ($15-30/M tokens).

## How It Works

```
Codex App → localhost:9090/v1/responses → Proxy → upstream/chat/completions → DeepSeek/GLM
```

The proxy converts OpenAI's Responses API format (used by Codex) to the Chat Completions API format (used by DeepSeek, GLM, etc.) and back.

## Quick Start

### 1. Install dependencies

```bash
pip install -r requirements.txt
```

### 2. Run as menu bar app

```bash
python app.py
```

A menu bar icon appears. Click it → Settings → enter your API key and select a provider preset.

### 3. Or run as CLI (headless)

```bash
# DeepSeek (default)
python proxy.py --upstream https://api.deepseek.com --api-key sk-xxx

# GLM (Zhipu)
python proxy.py --upstream https://open.bigmodel.cn/api/paas/v4 --api-key sk-xxx

# Using preset
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx
```

### 4. Configure Codex

Install the Codex config:

```bash
# Option A: via menu bar app → "Install Codex Config"
# Option B: manually copy config.toml.example to ~/.codex/config.toml
```

Or set environment variable:

```bash
export CODEX_PROXY_API_KEY="sk-xxx"
```

### 5. Start Codex

**Important:** Start the proxy first, then launch Codex.

## Supported Providers

| Provider | Upstream URL | Cost (vs GPT-5.5) |
|----------|-------------|-------------------|
| **DeepSeek V4 Pro** | `https://api.deepseek.com` | ~30-50x cheaper |
| **GLM-5.1 (Zhipu)** | `https://open.bigmodel.cn/api/paas/v4` | ~15-20x cheaper |
| **OpenRouter** | `https://openrouter.ai/api/v1` | Varies by model |
| **Custom** | Any OpenAI-compatible endpoint | — |

## Files

| File | Purpose |
|------|---------|
| `app.py` | macOS menu bar app (rumps) |
| `proxy.py` | Proxy server core (Flask) |
| `config.py` | Settings management |
| `setup.py` | py2app packaging |
| `requirements.txt` | Python dependencies |

## Architecture

The proxy handles:
- **Request conversion:** Responses API input → Chat Completions messages
- **Response conversion:** Chat Completions SSE → Responses API SSE events
- **Tool calling:** Full function_call/function_call_output support
- **Streaming:** Real-time SSE event translation
- **Reasoning:** DeepSeek reasoning_content storage for multi-turn context
- **Model mapping:** gpt-5.4 → deepseek-v4-pro, gpt-5.4-mini → deepseek-v4-flash

## Build macOS .app

```bash
pip install py2app
python setup.py py2app
# Output: dist/CodexProxy.app
```

## License

MIT
