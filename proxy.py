#!/usr/bin/env python3
"""
Codex Proxy — Bridges OpenAI Responses API <-> Chat Completions API or Anthropic Messages API.

Supports: DeepSeek, Zhipu GLM, OpenRouter, z.ai, and any OpenAI/Anthropic-compatible provider.
Auto-detects upstream API format (OpenAI vs Anthropic) based on URL.
Runs in a background thread for use with macOS menu bar app.

Usage:
  python proxy.py --upstream https://api.deepseek.com --api-key sk-xxx
  python proxy.py --upstream https://open.bigmodel.cn/api/paas/v4 --port 9090
  python proxy.py --upstream https://api.z.ai/api/anthropic --api-key sk-xxx
"""

import argparse
import hashlib
import json
import logging
import os
import sys
import threading
import time
import traceback
import uuid
from pathlib import Path

import requests as http_requests
from flask import Flask, Response, jsonify, request, stream_with_context

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
LOG_DIR = os.path.join(Path.home(), ".codexproxy")
os.makedirs(LOG_DIR, exist_ok=True)

LOG = os.path.join(LOG_DIR, "proxy.log")
RC_STORE = os.path.join(LOG_DIR, "reasoning_store.json")

logging.basicConfig(
    filename=LOG,
    level=logging.DEBUG,
    format="%(asctime)s %(levelname)s %(message)s",
)
log = logging.getLogger("proxy")

# ---------------------------------------------------------------------------
# Defaults & Presets
# ---------------------------------------------------------------------------

PRESETS: dict[str, dict] = {
    "DeepSeek V4 Pro": {
        "url": "https://api.deepseek.com",
        "models": {
            "gpt-5.4": "deepseek-v4-pro",
            "gpt-5.4-mini": "deepseek-v4-flash",
            "gpt-4o": "deepseek-v4-pro",
            "gpt-4o-mini": "deepseek-v4-flash",
        },
    },
    "OpenRouter": {
        "url": "https://openrouter.ai/api/v1",
        "models": {
            "gpt-5.4": "deepseek/deepseek-chat-v3-0324",
            "gpt-5.4-mini": "deepseek/deepseek-chat-v3-0324",
        },
    },
    "z.ai": {
        "url": "https://api.z.ai/api/anthropic",
        "api_type": "anthropic",
        "models": {
            "gpt-5.4": "claude-sonnet-4-20250514",
            "gpt-5.4-mini": "glm-5.1",
            "gpt-4o": "claude-sonnet-4-20250514",
            "gpt-4o-mini": "glm-5.1",
        },
    },
}

DEFAULT_PORT = 9090
DEFAULT_HOST = "127.0.0.1"

# ---------------------------------------------------------------------------
# State
# ---------------------------------------------------------------------------

app = Flask(__name__)
upstream_base: str = ""
model_map: dict[str, str] = {}
api_key_override: str = ""
upstream_api_type: str = "openai"  # "openai" or "anthropic"
server_thread: threading.Thread | None = None
_running = threading.Event()


def _is_anthropic() -> bool:
    """Auto-detect if upstream speaks Anthropic Messages API."""
    return upstream_api_type == "anthropic" or "/anthropic" in upstream_base

# ---------------------------------------------------------------------------
# Reasoning content store (persist to disk)
# ---------------------------------------------------------------------------

_rc_store: dict[str, str] = {}


def _load_rc_store():
    global _rc_store
    try:
        with open(RC_STORE, "r", encoding="utf-8") as f:
            _rc_store = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        _rc_store = {}


def _save_rc_store():
    with open(RC_STORE, "w", encoding="utf-8") as f:
        json.dump(_rc_store, f, ensure_ascii=False)


def _content_hash(text: str) -> str:
    return hashlib.md5(text.encode()).hexdigest()


def _store_reasoning(text: str, reasoning: str, tool_call_ids: list[str] | None = None):
    if not reasoning:
        return
    h = _content_hash(text)
    _rc_store[h] = reasoning
    if tool_call_ids:
        for tc_id in tool_call_ids:
            _rc_store[f"tc_{tc_id}"] = reasoning
    _save_rc_store()
    log.info("STORED reasoning hash=%s rc_len=%d tc_ids=%s", h[:12], len(reasoning), tool_call_ids)


def _lookup_reasoning(text: str) -> str:
    h = _content_hash(text)
    rc = _rc_store.get(h, "")
    if rc:
        log.info("FOUND reasoning hash=%s rc_len=%d", h[:12], len(rc))
    return rc


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _rid(prefix: str = "resp") -> str:
    return f"{prefix}_{uuid.uuid4().hex[:24]}"


def _sse(event: str, data: dict) -> str:
    return f"event: {event}\ndata: {json.dumps(data, ensure_ascii=False)}\n\n"


def _resolve_model(model: str) -> str:
    """Map OpenAI model name to upstream model."""
    if model in model_map:
        return model_map[model]
    # Fallback: any unknown gpt-* → first mapped model or passthrough
    if model.startswith("gpt-") and model_map:
        return list(model_map.values())[0]
    return model


# ---------------------------------------------------------------------------
# Request conversion: Responses API -> Chat Completions
# ---------------------------------------------------------------------------


def _convert_input(body: dict) -> list[dict]:
    messages: list[dict] = []
    if body.get("instructions"):
        messages.append({"role": "system", "content": body["instructions"]})

    inp = body.get("input", [])
    if isinstance(inp, str):
        messages.append({"role": "user", "content": inp})
        return messages
    if not isinstance(inp, list):
        return messages

    pending_tc: list[dict] = []
    pending_assistant: dict | None = None

    def _emit_pending():
        nonlocal pending_assistant
        if pending_assistant:
            messages.append(pending_assistant)
            pending_assistant = None

    def _emit_tc():
        nonlocal pending_tc
        if pending_tc:
            rc = ""
            for tc in pending_tc:
                rc = _rc_store.get(f"tc_{tc['id']}", "")
                if rc:
                    break
            msg = {"role": "assistant", "content": None, "tool_calls": list(pending_tc)}
            if rc:
                msg["reasoning_content"] = rc
            messages.append(msg)
            pending_tc = []

    for item in inp:
        if isinstance(item, str):
            _emit_pending()
            _emit_tc()
            messages.append({"role": "user", "content": item})
            continue
        if not isinstance(item, dict):
            continue
        t = item.get("type", "")

        if t == "function_call":
            pending_tc.append(
                {
                    "id": item.get("call_id", _rid("call")),
                    "type": "function",
                    "function": {
                        "name": item.get("name", ""),
                        "arguments": item.get("arguments", "{}"),
                    },
                }
            )
        elif t == "function_call_output":
            if pending_assistant:
                if pending_tc:
                    pending_assistant["tool_calls"] = list(pending_tc)
                    pending_tc = []
                messages.append(pending_assistant)
                pending_assistant = None
            else:
                _emit_tc()
            tool_output = item.get("output", "")
            if isinstance(tool_output, list):
                parts = []
                for p in tool_output:
                    if isinstance(p, str):
                        parts.append(p)
                    elif isinstance(p, dict) and p.get("type") in ("input_text", "text", "output_text"):
                        parts.append(p.get("text", ""))
                tool_output = "\n".join(parts)
            messages.append(
                {
                    "role": "tool",
                    "tool_call_id": item.get("call_id", ""),
                    "content": tool_output,
                }
            )
        else:
            _emit_pending()
            _emit_tc()
            role = item.get("role", "user")
            if role == "developer":
                role = "system"
            content = item.get("content", "")
            if isinstance(content, list):
                parts = []
                for p in content:
                    if isinstance(p, str):
                        parts.append(p)
                    elif isinstance(p, dict) and p.get("type") in ("input_text", "text", "output_text"):
                        parts.append(p.get("text", ""))
                content = "\n".join(parts)
            msg: dict = {}
            if role:
                msg["role"] = role
            if content is not None:
                msg["content"] = content
            if role == "assistant":
                stored_rc = _lookup_reasoning(content or "")
                if stored_rc:
                    msg["reasoning_content"] = stored_rc
                pending_assistant = msg
                continue
            if msg:
                messages.append(msg)

    _emit_pending()
    _emit_tc()
    return messages


def _convert_tools(tools: list | None) -> list | None:
    if not tools:
        return None
    out = []
    for tool in tools:
        if tool.get("type") == "function":
            func: dict = {"name": tool.get("name", "")}
            if tool.get("description"):
                func["description"] = tool["description"]
            if tool.get("parameters"):
                func["parameters"] = tool["parameters"]
            out.append({"type": "function", "function": func})
    return out or None


# ---------------------------------------------------------------------------
# Anthropic Messages API converters
# ---------------------------------------------------------------------------


def _convert_to_anthropic_messages(body: dict) -> tuple[list[dict], str]:
    """Convert Responses API input -> Anthropic Messages API format.
    Returns (messages, system_prompt).
    """
    system_prompt = ""
    if body.get("instructions"):
        system_prompt = body["instructions"]

    messages: list[dict] = []
    inp = body.get("input", [])
    if isinstance(inp, str):
        messages.append({"role": "user", "content": inp})
        return messages, system_prompt
    if not isinstance(inp, list):
        return messages, system_prompt

    for item in inp:
        if isinstance(item, str):
            messages.append({"role": "user", "content": item})
            continue
        if not isinstance(item, dict):
            continue
        t = item.get("type", "")
        role = item.get("role", "")

        if t == "function_call":
            # Anthropic tool_use block
            messages.append({
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": item.get("call_id", _rid("call")),
                    "name": item.get("name", ""),
                    "input": json.loads(item.get("arguments", "{}") or "{}"),
                }],
            })
        elif t == "function_call_output":
            messages.append({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": item.get("call_id", ""),
                    "content": item.get("output", ""),
                }],
            })
        elif role in ("user", "assistant"):
            content = item.get("content", "")
            if isinstance(content, list):
                parts = []
                for p in content:
                    if isinstance(p, str):
                        parts.append(p)
                    elif isinstance(p, dict) and p.get("type") in ("input_text", "text", "output_text"):
                        parts.append(p.get("text", ""))
                content = "\n".join(parts)
            if content:
                messages.append({"role": role, "content": content})

    return messages, system_prompt


def _convert_to_anthropic_tools(tools: list | None) -> list | None:
    """Convert Responses API tools -> Anthropic tool format."""
    if not tools:
        return None
    out = []
    for tool in tools:
        if tool.get("type") == "function":
            t: dict = {
                "name": tool.get("name", ""),
                "input_schema": tool.get("parameters", {"type": "object", "properties": {}}),
            }
            if tool.get("description"):
                t["description"] = tool["description"]
            out.append(t)
    return out or None


def _anthropic_to_responses(resp: dict, model: str) -> dict:
    """Convert Anthropic Messages API response -> Responses API format."""
    content_blocks = resp.get("content", [])
    text_parts = []
    tool_calls = []

    for block in content_blocks:
        if block.get("type") == "text":
            text_parts.append(block.get("text", ""))
        elif block.get("type") == "tool_use":
            tool_calls.append({
                "type": "function_call",
                "id": _rid("fc"),
                "call_id": block.get("id", _rid("call")),
                "name": block.get("name", ""),
                "arguments": json.dumps(block.get("input", {})),
                "status": "completed",
            })

    full_text = "".join(text_parts)
    output: list[dict] = []
    if full_text:
        output.append({
            "type": "message",
            "id": _rid("msg"),
            "status": "completed",
            "role": "assistant",
            "content": [{"type": "output_text", "text": full_text, "annotations": []}],
        })
    output.extend(tool_calls)

    usage = resp.get("usage", {})
    return {
        "id": _rid(),
        "object": "response",
        "created_at": int(time.time()),
        "model": model,
        "status": "completed",
        "output": output,
        "parallel_tool_calls": True,
        "usage": {
            "input_tokens": usage.get("input_tokens", 0),
            "output_tokens": usage.get("output_tokens", 0),
            "total_tokens": usage.get("input_tokens", 0) + usage.get("output_tokens", 0),
        },
        "metadata": {},
    }


def _stream_anthropic(cc_body: dict, headers: dict, model: str):
    """Stream from Anthropic Messages API -> Responses API SSE events."""
    resp_id = _rid()
    msg_id = _rid("msg")

    def gen():
        try:
            r = http_requests.post(
                f"{upstream_base}/v1/messages",
                json=cc_body,
                headers=headers,
                stream=True,
                timeout=180,
            )
            if r.status_code >= 400:
                log.error("ANTHROPIC UPSTREAM %d: %s", r.status_code, r.text[:500])
            r.raise_for_status()

            created = int(time.time())
            full_text = ""
            tool_calls_acc: list[dict] = []
            has_content = False
            input_tokens = 0
            output_tokens = 0

            for line in r.iter_lines(decode_unicode=True):
                if not line:
                    continue
                if line.startswith("event: "):
                    event_type = line[7:].strip()
                    continue
                if not line.startswith("data: "):
                    continue

                payload = line[6:]
                try:
                    data = json.loads(payload)
                except json.JSONDecodeError:
                    continue

                msg_type = data.get("type", "")

                if msg_type == "message_start":
                    input_tokens = data.get("message", {}).get("usage", {}).get("input_tokens", 0)

                elif msg_type == "content_block_start":
                    block = data.get("content_block", {})
                    if block.get("type") == "text":
                        if not has_content:
                            has_content = True
                            yield _sse("response.created", {
                                "type": "response.created",
                                "response": {"id": resp_id, "object": "response", "created_at": created,
                                             "model": model, "status": "in_progress", "output": [], "metadata": {}},
                            })
                            yield _sse("response.output_item.added", {
                                "type": "response.output_item.added",
                                "output_index": 0,
                                "item": {"type": "message", "id": msg_id, "status": "in_progress",
                                         "role": "assistant", "content": []},
                            })
                            yield _sse("response.content_part.added", {
                                "type": "response.content_part.added",
                                "output_index": 0, "content_index": 0,
                                "part": {"type": "output_text", "text": "", "annotations": []},
                            })
                    elif block.get("type") == "tool_use":
                        if not has_content:
                            has_content = True
                            yield _sse("response.created", {
                                "type": "response.created",
                                "response": {"id": resp_id, "object": "response", "created_at": created,
                                             "model": model, "status": "in_progress", "output": [], "metadata": {}},
                            })
                        tool_calls_acc.append({
                            "id": block.get("id", _rid("call")),
                            "name": block.get("name", ""),
                            "arguments": "",
                        })

                elif msg_type == "content_block_delta":
                    delta = data.get("delta", {})
                    if delta.get("type") == "text_delta":
                        text = delta.get("text", "")
                        if text:
                            full_text += text
                            yield _sse("response.output_text.delta", {
                                "type": "response.output_text.delta",
                                "output_index": 0, "content_index": 0, "delta": text,
                            })
                    elif delta.get("type") == "input_json_delta":
                        if tool_calls_acc:
                            tool_calls_acc[-1]["arguments"] += delta.get("partial_json", "")

                elif msg_type == "message_delta":
                    output_tokens = data.get("usage", {}).get("output_tokens", 0)

            # Close text
            if has_content and full_text:
                yield _sse("response.output_text.done", {
                    "type": "response.output_text.done",
                    "output_index": 0, "content_index": 0, "text": full_text,
                })
                yield _sse("response.output_item.done", {
                    "type": "response.output_item.done",
                    "output_index": 0,
                    "item": {"type": "message", "id": msg_id, "status": "completed",
                             "role": "assistant",
                             "content": [{"type": "output_text", "text": full_text, "annotations": []}]},
                })

            # Tool calls
            for idx, tc in enumerate(tool_calls_acc):
                fc_id = _rid("fc")
                oi = 1 + idx
                yield _sse("response.output_item.added", {
                    "type": "response.output_item.added",
                    "output_index": oi,
                    "item": {"type": "function_call", "id": fc_id, "call_id": tc["id"],
                             "name": tc["name"], "arguments": tc["arguments"], "status": "completed"},
                })
                yield _sse("response.output_item.done", {
                    "type": "response.output_item.done",
                    "output_index": oi,
                    "item": {"type": "function_call", "id": fc_id, "call_id": tc["id"],
                             "name": tc["name"], "arguments": tc["arguments"], "status": "completed"},
                })

            # Final response
            final_output = []
            if full_text:
                final_output.append({
                    "type": "message", "id": msg_id, "status": "completed",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": full_text, "annotations": []}],
                })
            for tc in tool_calls_acc:
                final_output.append({
                    "type": "function_call", "id": _rid("fc"), "call_id": tc["id"],
                    "name": tc["name"], "arguments": tc["arguments"], "status": "completed",
                })

            log.info("ANTHROPIC COMPLETED text=%d tools=%d", len(full_text), len(tool_calls_acc))
            yield _sse("response.completed", {
                "type": "response.completed",
                "response": {
                    "id": resp_id, "object": "response", "created_at": created,
                    "model": model, "status": "completed", "output": final_output,
                    "parallel_tool_calls": True,
                    "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens,
                              "total_tokens": input_tokens + output_tokens},
                    "metadata": {},
                },
            })

        except Exception as e:
            log.error("ANTHROPIC STREAM: %s\n%s", e, traceback.format_exc())
            yield _sse("error", {"type": "server_error", "message": str(e)})

    return Response(
        stream_with_context(gen()),
        content_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no", "Connection": "keep-alive"},
    )


# ---------------------------------------------------------------------------
# Response conversion: Chat Completions -> Responses API
# ---------------------------------------------------------------------------


def _cc_to_responses(cc: dict, model: str) -> dict:
    """Non-streaming: Chat Completions response -> Responses API response."""
    choice = cc.get("choices", [{}])[0]
    msg = choice.get("message", {})
    content = msg.get("content", "") or ""
    tool_calls = msg.get("tool_calls", [])

    output = []
    # Text message
    if content:
        output.append(
            {
                "type": "message",
                "id": _rid("msg"),
                "status": "completed",
                "role": "assistant",
                "content": [{"type": "output_text", "text": content, "annotations": []}],
            }
        )
    # Tool calls
    for tc in tool_calls:
        output.append(
            {
                "type": "function_call",
                "id": _rid("fc"),
                "call_id": tc.get("id", _rid("call")),
                "name": tc.get("function", {}).get("name", ""),
                "arguments": tc.get("function", {}).get("arguments", "{}"),
                "status": "completed",
            }
        )

    usage = cc.get("usage", {})
    return {
        "id": _rid(),
        "object": "response",
        "created_at": int(time.time()),
        "model": model,
        "status": "completed",
        "output": output,
        "parallel_tool_calls": True,
        "usage": {
            "input_tokens": usage.get("prompt_tokens", 0),
            "output_tokens": usage.get("completion_tokens", 0),
            "total_tokens": usage.get("total_tokens", 0),
        },
        "metadata": {},
    }


# ---------------------------------------------------------------------------
# Streaming: Chat Completions SSE -> Responses API SSE
# ---------------------------------------------------------------------------


def _stream(cc: dict, headers: dict, model: str):
    resp_id = _rid()
    msg_id = _rid("msg")

    def gen():
        try:
            r = http_requests.post(
                f"{upstream_base}/chat/completions",
                json=cc,
                headers=headers,
                stream=True,
                timeout=180,
            )
            if r.status_code >= 400:
                log.error("UPSTREAM %d: %s", r.status_code, r.text[:500])
            r.raise_for_status()

            created = int(time.time())
            full_text = ""
            full_reasoning = ""
            tool_calls_acc: dict[int, dict] = {}
            has_content = False
            final_usage: dict = {}

            for line in r.iter_lines(decode_unicode=True):
                if not line or not line.startswith("data: "):
                    continue
                payload = line[6:]
                if payload == "[DONE]":
                    break
                try:
                    chunk = json.loads(payload)
                except json.JSONDecodeError:
                    continue

                if chunk.get("usage"):
                    final_usage = chunk["usage"]

                choices = chunk.get("choices", [])
                if not choices:
                    continue

                delta = choices[0].get("delta", {})
                finish = choices[0].get("finish_reason")

                # Reasoning content (DeepSeek thinking)
                reasoning = delta.get("reasoning_content")
                if reasoning:
                    full_reasoning += reasoning

                content = delta.get("content")
                if content:
                    if not has_content:
                        has_content = True
                        yield _sse(
                            "response.created",
                            {
                                "type": "response.created",
                                "response": {
                                    "id": resp_id,
                                    "object": "response",
                                    "created_at": created,
                                    "model": model,
                                    "status": "in_progress",
                                    "output": [],
                                    "metadata": {},
                                },
                            },
                        )
                        yield _sse(
                            "response.output_item.added",
                            {
                                "type": "response.output_item.added",
                                "output_index": 0,
                                "item": {
                                    "type": "message",
                                    "id": msg_id,
                                    "status": "in_progress",
                                    "role": "assistant",
                                    "content": [],
                                },
                            },
                        )
                        yield _sse(
                            "response.content_part.added",
                            {
                                "type": "response.content_part.added",
                                "output_index": 0,
                                "content_index": 0,
                                "part": {"type": "output_text", "text": "", "annotations": []},
                            },
                        )
                    full_text += content
                    yield _sse(
                        "response.output_text.delta",
                        {
                            "type": "response.output_text.delta",
                            "output_index": 0,
                            "content_index": 0,
                            "delta": content,
                        },
                    )

                # Tool calls
                tc_delta = delta.get("tool_calls")
                if tc_delta:
                    if not has_content:
                        has_content = True
                        yield _sse(
                            "response.created",
                            {
                                "type": "response.created",
                                "response": {
                                    "id": resp_id,
                                    "object": "response",
                                    "created_at": created,
                                    "model": model,
                                    "status": "in_progress",
                                    "output": [],
                                    "metadata": {},
                                },
                            },
                        )
                    for tc in tc_delta:
                        idx = tc.get("index", 0)
                        if idx not in tool_calls_acc:
                            tool_calls_acc[idx] = {
                                "id": tc.get("id", _rid("call")),
                                "name": "",
                                "arguments": "",
                            }
                        if tc.get("id"):
                            tool_calls_acc[idx]["id"] = tc["id"]
                        fn = tc.get("function", {})
                        if fn.get("name"):
                            tool_calls_acc[idx]["name"] = fn["name"]
                        if fn.get("arguments"):
                            tool_calls_acc[idx]["arguments"] += fn["arguments"]

                if finish in ("stop", "tool_calls"):
                    break

            # Store reasoning
            tc_ids = (
                [tool_calls_acc[i]["id"] for i in sorted(tool_calls_acc)]
                if tool_calls_acc
                else None
            )
            _store_reasoning(full_text, full_reasoning, tc_ids)

            # Minimal response if no content
            if not has_content and not tool_calls_acc:
                yield _sse(
                    "response.created",
                    {
                        "type": "response.created",
                        "response": {
                            "id": resp_id,
                            "object": "response",
                            "created_at": created,
                            "model": model,
                            "status": "in_progress",
                            "output": [],
                            "metadata": {},
                        },
                    },
                )
                yield _sse(
                    "response.output_item.added",
                    {
                        "type": "response.output_item.added",
                        "output_index": 0,
                        "item": {
                            "type": "message",
                            "id": msg_id,
                            "status": "in_progress",
                            "role": "assistant",
                            "content": [],
                        },
                    },
                )
                yield _sse(
                    "response.content_part.added",
                    {
                        "type": "response.content_part.added",
                        "output_index": 0,
                        "content_index": 0,
                        "part": {"type": "output_text", "text": "", "annotations": []},
                    },
                )

            # Close text
            if has_content:
                yield _sse(
                    "response.output_text.done",
                    {
                        "type": "response.output_text.done",
                        "output_index": 0,
                        "content_index": 0,
                        "text": full_text,
                    },
                )
                yield _sse(
                    "response.output_item.done",
                    {
                        "type": "response.output_item.done",
                        "output_index": 0,
                        "item": {
                            "type": "message",
                            "id": msg_id,
                            "status": "completed",
                            "role": "assistant",
                            "content": [{"type": "output_text", "text": full_text, "annotations": []}],
                        },
                    },
                )

            # Tool call items
            for idx in sorted(tool_calls_acc):
                tc = tool_calls_acc[idx]
                fc_id = _rid("fc")
                oi = 1 + idx
                yield _sse(
                    "response.output_item.added",
                    {
                        "type": "response.output_item.added",
                        "output_index": oi,
                        "item": {
                            "type": "function_call",
                            "id": fc_id,
                            "call_id": tc["id"],
                            "name": tc["name"],
                            "arguments": tc["arguments"],
                            "status": "completed",
                        },
                    },
                )
                yield _sse(
                    "response.output_item.done",
                    {
                        "type": "response.output_item.done",
                        "output_index": oi,
                        "item": {
                            "type": "function_call",
                            "id": fc_id,
                            "call_id": tc["id"],
                            "name": tc["name"],
                            "arguments": tc["arguments"],
                            "status": "completed",
                        },
                    },
                )

            # Final output
            final_output: list[dict] = []
            if has_content:
                final_output.append(
                    {
                        "type": "message",
                        "id": msg_id,
                        "status": "completed",
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": full_text, "annotations": []}],
                    }
                )
            for idx in sorted(tool_calls_acc):
                tc = tool_calls_acc[idx]
                final_output.append(
                    {
                        "type": "function_call",
                        "id": _rid("fc"),
                        "call_id": tc["id"],
                        "name": tc["name"],
                        "arguments": tc["arguments"],
                        "status": "completed",
                    }
                )

            log.info(
                "COMPLETED text_len=%d tools=%d reasoning_len=%d",
                len(full_text),
                len(tool_calls_acc),
                len(full_reasoning),
            )
            yield _sse(
                "response.completed",
                {
                    "type": "response.completed",
                    "response": {
                        "id": resp_id,
                        "object": "response",
                        "created_at": created,
                        "model": model,
                        "status": "completed",
                        "output": final_output,
                        "parallel_tool_calls": True,
                        "usage": {
                            "input_tokens": final_usage.get("prompt_tokens", 0),
                            "output_tokens": final_usage.get("completion_tokens", 0),
                            "total_tokens": final_usage.get("total_tokens", 0),
                        },
                        "metadata": {},
                    },
                },
            )

        except Exception as e:
            log.error("STREAM ERROR: %s\n%s", e, traceback.format_exc())
            yield _sse("error", {"type": "server_error", "message": str(e)})

    return Response(
        stream_with_context(gen()),
        content_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no", "Connection": "keep-alive"},
    )


# ---------------------------------------------------------------------------
# Flask routes
# ---------------------------------------------------------------------------


@app.route("/v1/responses", methods=["POST"])
@app.route("/responses", methods=["POST"])
def handle():
    global upstream_base
    if not upstream_base:
        return jsonify({"error": {"message": "No upstream configured"}}), 503

    auth = request.headers.get("Authorization", "")
    key = auth.replace("Bearer ", "") if auth.startswith("Bearer ") else api_key_override
    if not key:
        return jsonify({"error": {"message": "No API key"}}), 401

    raw = request.get_data(as_text=True)
    # Save last request for debugging
    try:
        with open(os.path.join(LOG_DIR, "last_request.json"), "w", encoding="utf-8") as f:
            f.write(raw)
    except OSError:
        pass

    log.info("REQ: %s", raw[:500])

    try:
        body = json.loads(raw)
    except json.JSONDecodeError as e:
        return jsonify({"error": {"message": f"Bad JSON: {e}"}}), 400

    model = _resolve_model(body.get("model", "deepseek-v4-pro"))
    stream = body.get("stream", False)

    # ---- Route: Anthropic Messages API ----
    if _is_anthropic():
        messages, system_prompt = _convert_to_anthropic_messages(body)
        cc: dict = {"model": model, "messages": messages, "stream": stream, "max_tokens": body.get("max_tokens", 16384)}
        if system_prompt:
            cc["system"] = system_prompt
        cc_tools = _convert_to_anthropic_tools(body.get("tools"))
        if cc_tools:
            cc["tools"] = cc_tools
        if body.get("tool_choice"):
            tc = body["tool_choice"]
            # Anthropic requires tool_choice as dict, not string
            if isinstance(tc, str):
                if tc == "auto":
                    cc["tool_choice"] = {"type": "auto"}
                elif tc == "required":
                    cc["tool_choice"] = {"type": "any"}
                elif tc == "none":
                    pass  # omit tool_choice
            else:
                cc["tool_choice"] = tc
        for k in ("temperature", "top_p"):
            if k in body:
                cc[k] = body[k]

        headers = {
            "x-api-key": key,
            "Content-Type": "application/json",
            "anthropic-version": "2023-06-01",
        }

        log.info("FWD[anthropic]: model=%s msgs=%d tools=%d stream=%s",
                 cc.get("model"), len(cc.get("messages", [])), len(cc.get("tools", [])), stream)

        if stream:
            return _stream_anthropic(cc, headers, model)

        try:
            r = http_requests.post(f"{upstream_base}/v1/messages", json=cc, headers=headers, timeout=120)
            r.raise_for_status()
            return jsonify(_anthropic_to_responses(r.json(), model))
        except Exception as e:
            log.error("ANTHROPIC UPSTREAM: %s", e)
            return jsonify({"error": {"message": str(e)}}), 502

    # ---- Route: OpenAI Chat Completions API ----
    messages = _convert_input(body)
    cc = {"model": model, "messages": messages, "stream": stream}
    cc_tools = _convert_tools(body.get("tools"))
    if cc_tools:
        cc["tools"] = cc_tools
    if body.get("tool_choice"):
        cc["tool_choice"] = body["tool_choice"]
    for k in ("temperature", "top_p", "max_tokens", "max_completion_tokens"):
        if k in body:
            cc[k] = body[k]

    headers = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}

    log.info("FWD[openai]: model=%s msgs=%d tools=%d stream=%s",
             cc.get("model"), len(cc.get("messages", [])), len(cc.get("tools", [])), stream)

    if stream:
        return _stream(cc, headers, model)

    try:
        r = http_requests.post(f"{upstream_base}/chat/completions", json=cc, headers=headers, timeout=120)
        r.raise_for_status()
        return jsonify(_cc_to_responses(r.json(), model))
    except Exception as e:
        log.error("UPSTREAM: %s", e)
        return jsonify({"error": {"message": str(e)}}), 502


@app.route("/v1/models", methods=["GET"])
@app.route("/models", methods=["GET"])
def models():
    data = [{"id": m, "object": "model", "owned_by": "codex-proxy"} for m in set(model_map.values())]
    return jsonify({"object": "list", "data": data})


@app.route("/health", methods=["GET"])
def health():
    return jsonify({"status": "running", "upstream": upstream_base, "port": DEFAULT_PORT})


# ---------------------------------------------------------------------------
# Server control (for menu bar app)
# ---------------------------------------------------------------------------


def start_server(host: str = DEFAULT_HOST, port: int = DEFAULT_PORT):
    """Start Flask in a daemon thread."""
    global server_thread
    if server_thread and server_thread.is_alive():
        log.info("Server already running")
        return

    _running.set()
    server_thread = threading.Thread(
        target=lambda: app.run(host=host, port=port, threaded=True, use_reloader=False),
        daemon=True,
    )
    server_thread.start()
    log.info("Server started on %s:%d", host, port)


def stop_server():
    """Signal server stop."""
    _running.clear()
    log.info("Server stop requested")


def configure(upstream: str, models: dict[str, str] | None = None, api_key: str = "", api_type: str = ""):
    """Set upstream URL and model mapping."""
    global upstream_base, model_map, api_key_override, upstream_api_type
    upstream_base = upstream.rstrip("/")
    if models:
        model_map.update(models)
    if api_key:
        api_key_override = api_key
    if api_type:
        upstream_api_type = api_type
    log.info("Configured: upstream=%s api_type=%s models=%s", upstream_base, upstream_api_type, model_map)


# ---------------------------------------------------------------------------
# CLI standalone mode
# ---------------------------------------------------------------------------


def main():
    global upstream_base, model_map, api_key_override, upstream_api_type

    p = argparse.ArgumentParser(description="Codex Proxy — Responses API <-> Chat Completions / Anthropic")
    p.add_argument("--upstream", default="", help="Upstream API base URL")
    p.add_argument("--port", type=int, default=DEFAULT_PORT)
    p.add_argument("--host", default=DEFAULT_HOST)
    p.add_argument("--api-key", default="", help="API key (or set via env)")
    p.add_argument("--preset", choices=list(PRESETS.keys()), help="Use a built-in preset")
    args = p.parse_args()

    _load_rc_store()

    if args.preset:
        preset = PRESETS[args.preset]
        upstream_base = preset["url"]
        model_map = preset["models"]
        upstream_api_type = preset.get("api_type", "openai")
    elif args.upstream:
        upstream_base = args.upstream.rstrip("/")
        upstream_api_type = "anthropic" if "/anthropic" in upstream_base else "openai"
        model_map = {
            "gpt-5.4": "deepseek-v4-pro",
            "gpt-5.4-mini": "deepseek-v4-flash",
        }
    else:
        p.error("Specify --upstream or --preset")

    api_key_override = args.api_key or os.environ.get("CODEX_PROXY_API_KEY", "")

    print(f"[codex-proxy] http://{args.host}:{args.port} -> {upstream_base}")
    print(f"[codex-proxy] api_type: {upstream_api_type}")
    print(f"[codex-proxy] models: {model_map}")
    print(f"[codex-proxy] reasoning_store: {len(_rc_store)} entries")
    print(f"[codex-proxy] log: {LOG}")

    app.run(host=args.host, port=args.port, threaded=True, use_reloader=False)


if __name__ == "__main__":
    main()
