# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Python proxy server that routes OpenAI Codex Desktop (Responses API) through cheaper providers — DeepSeek, OpenRouter, MiniMax, Anthropic, LM Studio, Ollama. Includes desktop GUIs (pywebview and Tkinter).

## Commands

```bash
# GUI (pywebview — HTML/CSS via WebKit)
python app.py

# GUI (Tkinter — no extra deps)
python app_tkinter.py

# CLI proxy
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx

# Install deps
pip install -r requirements.txt

# Launch Codex CLI with proxy profile
bash launch-codex.sh
```

No tests, no linter, no build step. Proxy starts on `localhost:9090`.

## Architecture

### Request Flow

```
Codex Desktop → POST /v1/responses → proxy.py → provider API → response translation back
```

### proxy.py (1234 lines) — Key Sections

- **Model presets** (~L52-110): 7 built-in providers. Maps `gpt-5.4` → provider-specific model name.
- **API type detection**: Auto-detects OpenAI vs Anthropic from base URL.
- **OpenAI path** (~L201-318): `_convert_input()` → Chat Completions messages, `_cc_to_responses()` → Responses API format.
- **Anthropic path** (~L340-641): `_convert_to_anthropic_messages()`, `_anthropic_to_responses()`, `_stream_anthropic()`.
- **Streaming** (~L703-1027): `_stream()` — SSE event translation in real-time. Handles tool calls, reasoning content, text deltas.
- **Reasoning store** (~L133-175): DeepSeek `reasoning_content` saved by MD5 hash to `~/.codexproxy/reasoning_store.json`, re-injected into multi-turn via tool call IDs.
- **Endpoints**: `POST /v1/responses`, `GET /v1/models`, `GET /health`.

### config.py (257 lines)

- Settings: `~/.codexproxy/settings.json`
- API key: `~/.codexproxy/.api_key` (chmod 600)
- Codex config merge: `~/.codex/config.toml` — merge-safe, never overwrites, auto-backup.
- Restore: removes proxy entries from Codex config cleanly.

### API Key Priority

1. `Authorization` header → 2. `--api-key` CLI → 3. `CODEX_PROXY_API_KEY` env → 4. empty (no auth)

### File Locations

| What | Where |
|------|-------|
| Settings | `~/.codexproxy/settings.json` |
| API key | `~/.codexproxy/.api_key` |
| Reasoning store | `~/.codexproxy/reasoning_store.json` |
| Logs | `~/.codexproxy/proxy.log` |
| Debug dump | `~/.codexproxy/last_request.json` |
| Codex config | `~/.codex/config.toml` |

## Conventions

- Python 3.10+, deps: `flask`, `requests`, `pywebview` (for GUI).
- Server runs in daemon thread (for menu bar app integration).
- UUID generation: `uuid.uuid4().hex[:24]` for response IDs.
- Plugin compatibility: uses `env_key` not `experimental_bearer_token` so Codex stays in ChatGPT auth mode (marketplace plugins work).
- Comments and commit messages often in Russian.
