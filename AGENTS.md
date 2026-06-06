# Codex Proxy

Proxies OpenAI Responses API (Codex) → Chat Completions API (DeepSeek, MiniMax, OpenRouter) or Anthropic Messages API (z.ai). Two UI modes: pywebview desktop app (`app.py`) and headless CLI (`proxy.py`).

## Entrypoints

- `app.py` — pywebview desktop app (macOS WebKit). Entry: `python app.py`
- `proxy.py` — headless CLI. Entry: `python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx`
- `launch-codex.sh` — launches Codex.app with proxy profile (reads key from `~/.codexproxy/.api_key`)

## Providers (defined in `proxy.py:52-95`)

| Preset key | API type | URL |
|---|---|---|
| `z.ai` | anthropic | `https://api.z.ai/api/anthropic` |
| `DeepSeek V4 Pro` | openai | `https://api.deepseek.com` |
| `OpenRouter` | openai | `https://openrouter.ai/api/v1` |
| `MiniMax` | openai | `https://api.minimax.chat/v1` |
| `OpenCode` | openai | `https://opencode.ai/ru/go` |

OpenRouter validates keys and fetches model list via `GET {base}/models` with `Authorization: Bearer`. The UI (app.py) exposes `validate_key(preset, api_key)` and `fetch_models(preset, api_key)` to JS.

## Key Architecture

- Settings: `~/.codexproxy/settings.json` — JSON dict (preset, upstream, port, model_map, selected_model)
- API key: `~/.codexproxy/.api_key` — separate file, `chmod 600`
- Codex config: `~/.codex/config.toml` — merged automatically (never overwritten, with backup on each install)
- By default `/v1/responses` is mapped to `/chat/completions` (OpenAI-compatible). If URL contains `/anthropic` or `api_type` is `"anthropic"`, it maps to `/v1/messages` (Anthropic Messages API).

## Important Conventions

- API key is persisted separately from settings. `onProviderChange()` in JS clears the key on provider switch to prevent cross-provider key leaks. Saved key is restored after the clear during initial load.
- `start_proxy()` accepts optional `selected_model` param. If provided, all model_map values are overridden to that model name.
- `PRESETS` dict is duplicated in JS (`codex-proxy-redesign.html`) — must stay in sync with `proxy.py`.
- Model mapping: `gpt-5.4`, `gpt-5.4-mini`, `gpt-4o`, `gpt-4o-mini` → upstream model names. Unknown `gpt-*` models fall through to the first mapped value.
- `_resolve_model()` in proxy.py: `gpt-*` unknown → first mapped model; other models passthrough unchanged.

## Commands

```bash
pip install -r requirements.txt  # flask, requests, rumps
python proxy.py --preset "DeepSeek V4 Pro" --api-key sk-xxx  # headless
python app.py  # pywebview GUI
python setup.py py2app           # Build macOS .app bundle
```

## Build Artifacts

- `build.sh` builds native macOS app using `CodexProxyMenuBar.swift` + `app_native.py`
- `BUILD.md` has manual build steps if `build.sh` fails
- `setup.py` — py2app packaging for `app.py` (rumps-based menu bar app)

## Deploy

1. Start proxy first, then Codex. Launch order matters.
2. `launch-codex.sh` reads `~/.codexproxy/.api_key` and launches Codex with `--profile proxy`.
3. `CODEX_PROXY_API_KEY` env var can override the API key for CLI mode.
4. Config install merges into `~/.codex/config.toml`, never overwrites, creates timestamped backup.
