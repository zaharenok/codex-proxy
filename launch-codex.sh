#!/bin/bash
# Codex Proxy Launcher — starts Codex CLI with proxy profile
# Bypasses GUI "out of messages" subscription screen

export CODEX_PROXY_API_KEY="$(cat ~/.codexproxy/.api_key 2>/dev/null)"

if [ -z "$CODEX_PROXY_API_KEY" ]; then
  echo "Error: No API key found. Start Codex Proxy app first."
  exit 1
fi

exec /Applications/Codex.app/Contents/Resources/codex --profile proxy "$@"
