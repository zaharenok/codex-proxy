//! Codex Proxy — HTTP proxy server that bridges OpenAI Responses API
//! to Chat Completions API (OpenAI-compatible) or Anthropic Messages API.
//!
//! Ported from proxy.py

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::{get, post},
    Router,
};
use md5::{Digest, Md5};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::{error, info};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LOG_DIR: &str = ".codexproxy";
const RC_STORE_FILE: &str = "reasoning_store.json";

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Preset {
    pub url: &'static str,
    pub models_url: Option<&'static str>,
    pub api_type: &'static str,
    pub models: &'static [(&'static str, &'static str)],
}

pub fn get_presets() -> HashMap<&'static str, Preset> {
    let mut m = HashMap::new();
    m.insert(
        "DeepSeek V4 Pro",
        Preset {
            url: "https://api.deepseek.com",
            models_url: None,
            api_type: "openai",
            models: &[
                ("gpt-5.4", "deepseek-v4-pro"),
                ("gpt-5.4-mini", "deepseek-v4-flash"),
                ("gpt-4o", "deepseek-v4-pro"),
                ("gpt-4o-mini", "deepseek-v4-flash"),
            ],
        },
    );
    m.insert(
        "OpenRouter",
        Preset {
            url: "https://openrouter.ai/api/v1",
            models_url: None,
            api_type: "openai",
            models: &[
                ("gpt-5.4", "deepseek/deepseek-chat-v3-0324"),
                ("gpt-5.4-mini", "deepseek/deepseek-chat-v3-0324"),
            ],
        },
    );
    m.insert(
        "MiniMax",
        Preset {
            url: "https://api.minimax.chat/v1",
            models_url: None,
            api_type: "openai",
            models: &[
                ("gpt-5.4", "minimax-text-01"),
                ("gpt-5.4-mini", "minimax-m2"),
                ("gpt-4o", "minimax-text-01"),
                ("gpt-4o-mini", "minimax-m2"),
            ],
        },
    );
    m.insert(
        "OpenCode Go",
        Preset {
            url: "https://opencode.ai/zen/go/v1",
            models_url: Some("https://opencode.ai/zen/go/v1"),
            api_type: "openai",
            models: &[
                ("gpt-5.6", "deepseek-v4-pro"),
                ("gpt-5.5", "deepseek-v4-pro"),
                ("gpt-5.4", "deepseek-v4-pro"),
                ("gpt-5.4-mini", "deepseek-v4-flash"),
                ("gpt-4o", "deepseek-v4-pro"),
                ("gpt-4o-mini", "deepseek-v4-flash"),
            ],
        },
    );
    m.insert(
        "LM Studio",
        Preset {
            url: "http://localhost:1234/v1",
            models_url: None,
            api_type: "openai",
            models: &[("gpt-5.4", "local-model"), ("gpt-5.4-mini", "local-model")],
        },
    );
    m.insert(
        "Ollama",
        Preset {
            url: "http://localhost:11434/v1",
            models_url: None,
            api_type: "openai",
            models: &[("gpt-5.4", "local-model"), ("gpt-5.4-mini", "local-model")],
        },
    );
    m.insert(
        "z.ai",
        Preset {
            url: "https://api.z.ai/api/anthropic",
            models_url: Some("https://api.z.ai/api/anthropic/v1"),
            api_type: "anthropic",
            models: &[
                ("gpt-5.4", "claude-sonnet-4-20250514"),
                ("gpt-5.4-mini", "glm-5.1"),
                ("gpt-4o", "claude-sonnet-4-20250514"),
                ("gpt-4o-mini", "glm-5.1"),
            ],
        },
    );
    m
}

// ---------------------------------------------------------------------------
// Reasoning content store
// ---------------------------------------------------------------------------

pub struct ReasoningStore {
    store: HashMap<String, String>,
    path: PathBuf,
}

impl ReasoningStore {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("HOME not found");
        let path = home.join(LOG_DIR).join(RC_STORE_FILE);
        let store = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { store, path }
    }

    fn content_hash(text: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn store_reasoning(&mut self, text: &str, reasoning: &str, tool_call_ids: Option<&[String]>) {
        if reasoning.is_empty() {
            return;
        }
        let h = Self::content_hash(text);
        self.store.insert(h.clone(), reasoning.to_string());
        if let Some(ids) = tool_call_ids {
            for tc_id in ids {
                self.store.insert(format!("tc_{}", tc_id), reasoning.to_string());
            }
        }
        // Persist
        if let Ok(content) = serde_json::to_string(&self.store) {
            let _ = fs::write(&self.path, &content);
        }
        info!(
            "STORED reasoning hash={} rc_len={} tc_ids={:?}",
            &h[..12],
            reasoning.len(),
            tool_call_ids
        );
    }

    pub fn lookup_reasoning(&self, text: &str) -> String {
        let h = Self::content_hash(text);
        let rc = self.store.get(&h).cloned().unwrap_or_default();
        if !rc.is_empty() {
            info!("FOUND reasoning hash={} rc_len={}", &h[..12], rc.len());
        }
        rc
    }
}

// ---------------------------------------------------------------------------
// Shared proxy state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ProxyState {
    pub upstream_base: Arc<RwLock<String>>,
    pub model_map: Arc<RwLock<HashMap<String, String>>>,
    pub api_key_override: Arc<RwLock<String>>,
    pub upstream_api_type: Arc<RwLock<String>>,
    pub rc_store: Arc<RwLock<ReasoningStore>>,
    pub http_client: Client,
}

impl ProxyState {
    pub fn global() -> &'static ProxyState {
        static GLOBAL: OnceLock<ProxyState> = OnceLock::new();
        GLOBAL.get_or_init(|| ProxyState::new())
    }

    pub fn new() -> Self {
        Self {
            upstream_base: Arc::new(RwLock::new(String::new())),
            model_map: Arc::new(RwLock::new(HashMap::new())),
            api_key_override: Arc::new(RwLock::new(String::new())),
            upstream_api_type: Arc::new(RwLock::new("openai".to_string())),
            rc_store: Arc::new(RwLock::new(ReasoningStore::new())),
            http_client: Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    pub async fn is_anthropic(&self) -> bool {
        let api_type = self.upstream_api_type.read().await;
        let base = self.upstream_base.read().await;
        *api_type == "anthropic" || base.contains("/anthropic")
    }

    pub async fn resolve_model(&self, model: &str) -> String {
        let map = self.model_map.read().await;
        if let Some(mapped) = map.get(model) {
            return mapped.clone();
        }
        // Fallback: ANY unknown model name (Codex may send a ChatGPT-account
        // default like "gpt-5.5" or "gpt-5.6" we didn't pre-register) →
        // first mapped value in the preset.
        if !map.is_empty() {
            return map.values().next().cloned().unwrap_or_else(|| model.to_string());
        }
        model.to_string()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rid(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().to_string().replace("-", "")[..24].to_string())
}

#[allow(dead_code)]
fn sse(event: &str, data: &Value) -> String {
    format!(
        "event: {}\ndata: {}\n\n",
        event,
        serde_json::to_string(data).unwrap_or_default()
    )
}

fn extract_content(item: &Value) -> String {
    match item["content"] {
        Value::String(ref s) => s.clone(),
        Value::Array(ref arr) => {
            let mut parts = Vec::new();
            for p in arr {
                if let Value::String(s) = p {
                    parts.push(s.clone());
                } else if let Value::Object(obj) = p {
                    if matches!(obj.get("type").and_then(|v| v.as_str()), Some("input_text" | "text" | "output_text")) {
                        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                            parts.push(text.to_string());
                        }
                    }
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

fn extract_tool_output(item: &Value) -> String {
    let output = item.get("output");
    match output {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => {
            let mut parts = Vec::new();
            for p in arr {
                if let Value::String(s) = p {
                    parts.push(s.clone());
                } else if let Value::Object(obj) = p {
                    if matches!(obj.get("type").and_then(|v| v.as_str()), Some("input_text" | "text" | "output_text")) {
                        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                            parts.push(text.to_string());
                        }
                    }
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Request conversion: Responses API -> Chat Completions
// ---------------------------------------------------------------------------

fn convert_input(body: &Value, rc_store: &ReasoningStore) -> Vec<Value> {
    let mut messages: Vec<Value> = Vec::new();

    // Instructions → system message
    if let Some(instructions) = body.get("instructions").and_then(|v| v.as_str()) {
        messages.push(json!({"role": "system", "content": instructions}));
    }

    let input = body.get("input");
    let items = match input {
        Some(Value::String(s)) => {
            messages.push(json!({"role": "user", "content": s}));
            return messages;
        }
        Some(Value::Array(arr)) => arr.clone(),
        _ => return messages,
    };

    let mut pending_assistant: Option<Value> = None;
    let mut pending_tc: Vec<Value> = Vec::new();

    for item in &items {
        if let Value::String(s) = item {
            // Flush pending
            if let Some(pa) = pending_assistant.take() {
                messages.push(pa);
            }
            if !pending_tc.is_empty() {
                let rc = pending_tc.iter()
                    .find_map(|tc| {
                        tc.get("id").and_then(|id| {
                            let store = rc_store.store.get(&format!("tc_{}", id.as_str().unwrap_or("")));
                            store.cloned()
                        })
                    })
                    .unwrap_or_default();
                let mut msg = json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": pending_tc
                });
                if !rc.is_empty() {
                    msg["reasoning_content"] = json!(rc);
                }
                messages.push(msg);
                pending_tc.clear();
            }
            messages.push(json!({"role": "user", "content": s}));
            continue;
        }

        let t = item.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match t {
            "function_call" => {
                pending_tc.push(json!({
                    "id": item.get("call_id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| rid("call")),
                    "type": "function",
                    "function": {
                        "name": item.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "arguments": item.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}"),
                    }
                }));
            }
            "function_call_output" => {
                if let Some(mut pa) = pending_assistant.take() {
                    if !pending_tc.is_empty() {
                        pa["tool_calls"] = json!(pending_tc);
                        pending_tc.clear();
                    }
                    messages.push(pa);
                } else {
                    if !pending_tc.is_empty() {
                        let rc = pending_tc.iter()
                            .find_map(|tc| {
                                tc.get("id").and_then(|id| {
                                    let store = rc_store.store.get(&format!("tc_{}", id.as_str().unwrap_or("")));
                                    store.cloned()
                                })
                            })
                            .unwrap_or_default();
                        let mut msg = json!({
                            "role": "assistant",
                            "content": null,
                            "tool_calls": pending_tc
                        });
                        if !rc.is_empty() {
                            msg["reasoning_content"] = json!(rc);
                        }
                        messages.push(msg);
                        pending_tc.clear();
                    }
                }
                let tool_output = extract_tool_output(item);
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": item.get("call_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "content": tool_output,
                }));
            }
            _ => {
                // Flush pending
                if let Some(pa) = pending_assistant.take() {
                    messages.push(pa);
                }
                if !pending_tc.is_empty() {
                    let rc = pending_tc.iter()
                        .find_map(|tc| {
                            tc.get("id").and_then(|id| {
                                let store = rc_store.store.get(&format!("tc_{}", id.as_str().unwrap_or("")));
                                store.cloned()
                            })
                        })
                        .unwrap_or_default();
                    let mut msg = json!({
                        "role": "assistant",
                        "content": null,
                        "tool_calls": pending_tc
                    });
                    if !rc.is_empty() {
                        msg["reasoning_content"] = json!(rc);
                    }
                    messages.push(msg);
                    pending_tc.clear();
                }

                let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                let role = if role == "developer" { "system" } else { role };
                let content = extract_content(item);

                if role == "assistant" {
                    let stored_rc = rc_store.lookup_reasoning(&content);
                    let mut msg = json!({"role": "assistant", "content": content});
                    if !stored_rc.is_empty() {
                        msg["reasoning_content"] = json!(stored_rc);
                    }
                    pending_assistant = Some(msg);
                    continue;
                }

                if !role.is_empty() && !content.is_empty() {
                    messages.push(json!({"role": role, "content": content}));
                }
            }
        }
    }

    // Final flush
    if let Some(pa) = pending_assistant.take() {
        messages.push(pa);
    }
    if !pending_tc.is_empty() {
        let msg = json!({
            "role": "assistant",
            "content": null,
            "tool_calls": pending_tc
        });
        messages.push(msg);
    }

    messages
}

fn convert_tools(body: &Value) -> Option<Value> {
    let tools = body.get("tools")?;
    let arr = tools.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let out: Vec<Value> = arr
        .iter()
        .filter_map(|tool| {
            if tool.get("type").and_then(|v| v.as_str()) == Some("function") {
                let mut func = json!({
                    "name": tool.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                });
                if let Some(desc) = tool.get("description").and_then(|v| v.as_str()) {
                    func["description"] = json!(desc);
                }
                if let Some(params) = tool.get("parameters") {
                    func["parameters"] = params.clone();
                }
                Some(json!({"type": "function", "function": func}))
            } else {
                None
            }
        })
        .collect();
    if out.is_empty() {
        None
    } else {
        Some(json!(out))
    }
}

// ---------------------------------------------------------------------------
// Anthropic converters
// ---------------------------------------------------------------------------

fn convert_to_anthropic_messages(body: &Value) -> (Vec<Value>, String) {
    let system_prompt = body
        .get("instructions")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let input = body.get("input");
    let items = match input {
        Some(Value::String(s)) => return (vec![json!({"role": "user", "content": s})], system_prompt),
        Some(Value::Array(arr)) => arr.clone(),
        _ => return (Vec::new(), system_prompt),
    };

    let mut messages: Vec<Value> = Vec::new();
    for item in &items {
        if let Value::String(s) = item {
            messages.push(json!({"role": "user", "content": s}));
            continue;
        }
        let t = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let role = item.get("role").and_then(|v| v.as_str()).unwrap_or("");

        match t {
            "function_call" => {
                let args = item.get("arguments").and_then(|v| v.as_str()).unwrap_or("{}");
                let parsed_args: Value = serde_json::from_str(args).unwrap_or(json!({}));
                messages.push(json!({
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                            "id": item.get("call_id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| rid("call")),
                        "name": item.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        "input": parsed_args,
                    }]
                }));
            }
            "function_call_output" => {
                messages.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": item.get("call_id").and_then(|v| v.as_str()).unwrap_or(""),
                        "content": item.get("output").and_then(|v| v.as_str()).unwrap_or(""),
                    }]
                }));
            }
            _ if role == "user" || role == "assistant" => {
                let content = extract_content(item);
                if !content.is_empty() {
                    messages.push(json!({"role": role, "content": content}));
                }
            }
            _ => {}
        }
    }
    (messages, system_prompt)
}

fn convert_to_anthropic_tools(body: &Value) -> Option<Value> {
    let tools = body.get("tools")?;
    let arr = tools.as_array()?;
    if arr.is_empty() {
        return None;
    }
    let out: Vec<Value> = arr
        .iter()
        .filter_map(|tool| {
            if tool.get("type").and_then(|v| v.as_str()) != Some("function") {
                return None;
            }
            let mut t = json!({
                "name": tool.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "input_schema": tool.get("parameters").cloned().unwrap_or(json!({"type": "object", "properties": {}})),
            });
            if let Some(desc) = tool.get("description").and_then(|v| v.as_str()) {
                t["description"] = json!(desc);
            }
            Some(t)
        })
        .collect();
    if out.is_empty() {
        None
    } else {
        Some(json!(out))
    }
}

fn anthropic_to_responses(resp: &Value, model: &str) -> Value {
    let content_blocks = resp.get("content").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut text_parts = Vec::new();
    let mut tool_calls = Vec::new();

    for block in &content_blocks {
        match block.get("type").and_then(|v| v.as_str()) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    text_parts.push(text.to_string());
                }
            }
            Some("tool_use") => {
                let input = block.get("input").cloned().unwrap_or(json!({}));
                tool_calls.push(json!({
                    "type": "function_call",
                    "id": rid("fc"),
                    "call_id": block.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| rid("call")),
                    "name": block.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "arguments": serde_json::to_string(&input).unwrap_or_default(),
                    "status": "completed",
                }));
            }
            _ => {}
        }
    }

    let full_text = text_parts.join("");
    let mut output: Vec<Value> = Vec::new();
    if !full_text.is_empty() {
        output.push(json!({
            "type": "message",
            "id": rid("msg"),
            "status": "completed",
            "role": "assistant",
            "content": [{"type": "output_text", "text": full_text, "annotations": []}],
        }));
    }
    output.extend(tool_calls);

    let usage = resp.get("usage").cloned().unwrap_or(json!({}));
    let in_tokens = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let out_tokens = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

    json!({
        "id": rid("resp"),
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "model": model,
        "status": "completed",
        "output": output,
        "parallel_tool_calls": true,
        "usage": {
            "input_tokens": in_tokens,
            "output_tokens": out_tokens,
            "total_tokens": in_tokens + out_tokens,
        },
        "metadata": {},
    })
}

// ---------------------------------------------------------------------------
// Response conversion
// ---------------------------------------------------------------------------

fn cc_to_responses(cc: &Value, model: &str) -> Value {
    let choice = cc.get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or_default();
    let msg = choice.get("message").cloned().unwrap_or_default();
    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let tool_calls = msg.get("tool_calls").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut output: Vec<Value> = Vec::new();

    if !content.is_empty() {
        output.push(json!({
            "type": "message",
            "id": rid("msg"),
            "status": "completed",
            "role": "assistant",
            "content": [{"type": "output_text", "text": content, "annotations": []}],
        }));
    }

    for tc in &tool_calls {
        output.push(json!({
            "type": "function_call",
            "id": rid("fc"),
            "call_id": tc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| rid("call")),
            "name": tc.get("function").and_then(|f| f.get("name")).and_then(|v| v.as_str()).unwrap_or(""),
            "arguments": tc.get("function").and_then(|f| f.get("arguments")).and_then(|v| v.as_str()).unwrap_or("{}"),
            "status": "completed",
        }));
    }

    let usage = cc.get("usage").cloned().unwrap_or(json!({}));
    let in_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    let out_tokens = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

    json!({
        "id": rid("resp"),
        "object": "response",
        "created_at": chrono::Utc::now().timestamp(),
        "model": model,
        "status": "completed",
        "output": output,
        "parallel_tool_calls": true,
        "usage": {
            "input_tokens": in_tokens,
            "output_tokens": out_tokens,
            "total_tokens": in_tokens + out_tokens,
        },
        "metadata": {},
    })
}

// ---------------------------------------------------------------------------
// HTTP handlers
// ---------------------------------------------------------------------------

pub async fn handle_responses(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: String,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    let upstream_base = state.upstream_base.read().await.clone();
    if upstream_base.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": {"message": "No upstream configured"}})),
        ));
    }

    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let key = if auth.starts_with("Bearer ") {
        auth.trim_start_matches("Bearer ").to_string()
    } else if !state.api_key_override.read().await.is_empty() {
        state.api_key_override.read().await.clone()
    } else if let Ok(env_key) = std::env::var("CODEX_PROXY_API_KEY") {
        if !env_key.is_empty() { env_key } else { String::new() }
    } else {
        String::new()
    };
    if key.is_empty() {
        info!("REQ REJECTED: no API key (auth={:?}, upstream={})", auth, upstream_base);
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"error": {"message": "No API key"}}))));
    }
    info!("REQ AUTH: key_len={}", key.len());

    let body_val: Value = serde_json::from_str(&body).map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": {"message": format!("Bad JSON: {}", e)}})))
    })?;

    let model = state.resolve_model(
        body_val.get("model").and_then(|v| v.as_str()).unwrap_or("deepseek-v4-pro"),
    ).await;
    let stream = body_val.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

    info!("REQ: {}", &body[..body.len().min(500)]);

    // ---- Anthropic route ----
    if state.is_anthropic().await {
        return handle_anthropic(state, body_val, model, stream, key, upstream_base.clone()).await;
    }

    // ---- OpenAI Chat Completions route ----
    let rc_store = state.rc_store.read().await;
    let messages = convert_input(&body_val, &rc_store);
    drop(rc_store);

    let mut cc = json!({
        "model": model,
        "messages": messages,
        "stream": stream,
    });

    if let Some(tools) = convert_tools(&body_val) {
        cc["tools"] = tools;
    }
    if let Some(tc) = body_val.get("tool_choice") {
        cc["tool_choice"] = tc.clone();
    }
    for k in &["temperature", "top_p", "max_tokens", "max_completion_tokens"] {
        if let Some(v) = body_val.get(*k) {
            cc[*k] = v.clone();
        }
    }
    if let Some(reasoning) = body_val.get("reasoning").and_then(|v| v.as_object()) {
        if let Some(effort) = reasoning.get("effort").and_then(|v| v.as_str()) {
            cc["reasoning_effort"] = json!(effort);
        }
    }

    let req_headers = reqwest::header::HeaderMap::from_iter(vec![
        (reqwest::header::AUTHORIZATION, format!("Bearer {}", key).parse().unwrap()),
        (reqwest::header::CONTENT_TYPE, "application/json".parse().unwrap()),
    ]);

    if stream {
        Ok(stream_openai(state, cc, req_headers, model, upstream_base.clone()).await)
    } else {
        let resp = state.http_client
            .post(format!("{}/chat/completions", upstream_base))
            .headers(req_headers)
            .json(&cc)
            .send()
            .await
            .map_err(|e| {
                error!("UPSTREAM: {}", e);
                (StatusCode::BAD_GATEWAY, Json(json!({"error": {"message": e.to_string()}})))
            })?;

        let http_status = resp.status();
        if http_status.is_success() {
            let data: Value = resp.json().await.map_err(|e| {
                (StatusCode::BAD_GATEWAY, Json(json!({"error": {"message": e.to_string()}})))
            })?;
            Ok(Json(cc_to_responses(&data, &model)).into_response())
        } else {
            let text = resp.text().await.unwrap_or_default();
            error!("UPSTREAM {}: {}", http_status, &text[..text.len().min(500)]);
            Err((StatusCode::BAD_GATEWAY, Json(json!({"error": {"message": text}}))))
        }
    }
}

async fn handle_anthropic(
    state: ProxyState,
    body_val: Value,
    model: String,
    stream: bool,
    key: String,
    upstream_base: String,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    let (messages, system_prompt) = convert_to_anthropic_messages(&body_val);

    let mut cc = json!({
        "model": model,
        "messages": messages,
        "stream": stream,
        "max_tokens": body_val.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(16384),
    });

    if !system_prompt.is_empty() {
        cc["system"] = json!(system_prompt);
    }
    if let Some(tools) = convert_to_anthropic_tools(&body_val) {
        cc["tools"] = tools;
    }
    if let Some(tc) = body_val.get("tool_choice") {
        if let Some(s) = tc.as_str() {
            match s {
                "auto" => cc["tool_choice"] = json!({"type": "auto"}),
                "required" => cc["tool_choice"] = json!({"type": "any"}),
                "none" => {} // omit
                _ => cc["tool_choice"] = json!({"type": s}),
            }
        } else {
            cc["tool_choice"] = tc.clone();
        }
    }
    for k in &["temperature", "top_p"] {
        if let Some(v) = body_val.get(*k) {
            cc[*k] = v.clone();
        }
    }

    info!("FWD[anthropic]: model={} msgs={} stream={}", cc["model"], messages.len(), stream);

    let mut req_headers = reqwest::header::HeaderMap::new();
    req_headers.insert("x-api-key", key.parse().unwrap());
    req_headers.insert("Content-Type", "application/json".parse().unwrap());
    req_headers.insert("anthropic-version", "2023-06-01".parse().unwrap());

    if stream {
        Ok(stream_anthropic(state, cc, req_headers, model, upstream_base).await)
    } else {
        let resp = state.http_client
            .post(format!("{}/v1/messages", upstream_base))
            .headers(req_headers)
            .json(&cc)
            .send()
            .await
            .map_err(|e| {
                error!("ANTHROPIC UPSTREAM: {}", e);
                (StatusCode::BAD_GATEWAY, Json(json!({"error": {"message": e.to_string()}})))
            })?;

        if resp.status().is_success() {
            let data: Value = resp.json().await.map_err(|e| {
                (StatusCode::BAD_GATEWAY, Json(json!({"error": {"message": e.to_string()}})))
            })?;
            Ok(Json(anthropic_to_responses(&data, &model)).into_response())
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            error!("ANTHROPIC UPSTREAM {}: {}", status, &text[..text.len().min(500)]);
            Err((StatusCode::BAD_GATEWAY, Json(json!({"error": {"message": text}}))))
        }
    }
}

// ---------------------------------------------------------------------------
// SSE Streaming for OpenAI-compatible upstream
// ---------------------------------------------------------------------------

async fn stream_openai(
    state: ProxyState,
    cc: Value,
    headers: reqwest::header::HeaderMap,
    model: String,
    upstream_base: String,
) -> axum::response::Response {
    let resp_id = rid("resp");
    let msg_id = rid("msg");

    let stream = async_stream::stream! {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .unwrap_or_else(|_| Client::new());

        let resp = match http_client
            .post(format!("{}/chat/completions", upstream_base))
            .headers(headers)
            .json(&cc)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("STREAM UPSTREAM: {}", e);
                yield Ok::<_, axum::Error>(Event::default().event("error").data(format!("{}", e)));
                let err_resp = json!({
                    "type": "response.completed",
                    "response": {
                        "id": &resp_id, "object": "response", "created_at": chrono::Utc::now().timestamp(),
                        "model": &model, "status": "failed", "output": [],
                        "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                        "metadata": {},
                    }
                });
                yield Ok(Event::default().event("response.completed").data(serde_json::to_string(&err_resp).unwrap()));
                return;
            }
        };

        let http_status = resp.status();
        if !http_status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            error!("UPSTREAM {}: {}", http_status, &text[..text.len().min(500)]);
            yield Ok(Event::default().event("error").data(format!("Upstream {}: {}", http_status, &text[..text.len().min(500)])));
            yield Ok(Event::default().event("response.completed").data(serde_json::to_string(&json!({
                "type": "response.completed",
                "response": {
                    "id": &resp_id, "object": "response", "created_at": chrono::Utc::now().timestamp(),
                    "model": &model, "status": "failed", "output": [],
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                    "metadata": {},
                }
            })).unwrap()));
            return;
        }

        let created = chrono::Utc::now().timestamp();
        let mut full_text = String::new();
        let mut full_reasoning = String::new();
        let mut tool_calls_acc: HashMap<usize, Value> = HashMap::new();
        let mut has_content = false;
        let mut final_usage = json!({});

        let mut stream_resp = resp.bytes_stream();
        use futures::StreamExt;
        let mut buf = String::new();

        while let Some(chunk) = stream_resp.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(_) => break,
            };
            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = buf.find('\n') {
                let line = buf[..line_end].trim().to_string();
                buf = buf[line_end + 1..].to_string();

                if !line.starts_with("data: ") {
                    continue;
                }
                let payload = &line[6..];
                if payload == "[DONE]" {
                    break;
                }

                let chunk_val: Value = match serde_json::from_str(payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if let Some(usage) = chunk_val.get("usage") {
                    final_usage = usage.clone();
                }

                let choices = chunk_val.get("choices")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                if choices.is_empty() {
                    continue;
                }

                let delta = choices[0].get("delta").cloned().unwrap_or_default();
                let finish = choices[0].get("finish_reason").and_then(|v| v.as_str());

                // Reasoning content
                if let Some(reasoning) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                    full_reasoning.push_str(reasoning);
                }

                // Content delta
                if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                    if !has_content {
                        has_content = true;
                        yield Ok(Event::default().event("response.created").data(serde_json::to_string(&json!({
                            "type": "response.created",
                            "response": {"id": &resp_id, "object": "response", "created_at": created,
                                         "model": &model, "status": "in_progress", "output": [], "metadata": {}}
                        })).unwrap()));
                        yield Ok(Event::default().event("response.output_item.added").data(serde_json::to_string(&json!({
                            "type": "response.output_item.added",
                            "output_index": 0,
                            "item": {"type": "message", "id": &msg_id, "status": "in_progress",
                                     "role": "assistant", "content": []}
                        })).unwrap()));
                        yield Ok(Event::default().event("response.content_part.added").data(serde_json::to_string(&json!({
                            "type": "response.content_part.added",
                            "output_index": 0, "content_index": 0,
                            "part": {"type": "output_text", "text": "", "annotations": []}
                        })).unwrap()));
                    }
                    full_text.push_str(content);
                    yield Ok(Event::default().event("response.output_text.delta").data(serde_json::to_string(&json!({
                        "type": "response.output_text.delta",
                        "output_index": 0, "content_index": 0, "delta": content
                    })).unwrap()));
                }

                // Tool calls
                if let Some(tc_delta) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                    if !has_content {
                        has_content = true;
                        yield Ok(Event::default().event("response.created").data(serde_json::to_string(&json!({
                            "type": "response.created",
                            "response": {"id": &resp_id, "object": "response", "created_at": created,
                                         "model": &model, "status": "in_progress", "output": [], "metadata": {}}
                        })).unwrap()));
                    }
                    for tc in tc_delta {
                        let idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        let entry = tool_calls_acc.entry(idx).or_insert_with(|| json!({
                            "id": rid("call"),
                            "name": "",
                            "arguments": "",
                        }));
                        if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                            entry["id"] = json!(id);
                        }
                        if let Some(fn_val) = tc.get("function") {
                            if let Some(name) = fn_val.get("name").and_then(|v| v.as_str()) {
                                entry["name"] = json!(name);
                            }
                            if let Some(args) = fn_val.get("arguments").and_then(|v| v.as_str()) {
                                let current = entry["arguments"].as_str().unwrap_or("").to_string();
                                entry["arguments"] = json!(current + args);
                            }
                        }
                    }
                }

                if matches!(finish, Some("stop" | "tool_calls")) {
                    break;
                }
            }
        }

        // Store reasoning
        let tc_ids: Vec<String> = {
            let mut ids = Vec::new();
            let mut keys: Vec<_> = tool_calls_acc.keys().collect();
            keys.sort();
            for k in keys {
                if let Some(id) = tool_calls_acc[k].get("id").and_then(|v| v.as_str()) {
                    ids.push(id.to_string());
                }
            }
            ids
        };
        {
            let mut store = state.rc_store.write().await;
            store.store_reasoning(&full_text, &full_reasoning, if tc_ids.is_empty() { None } else { Some(&tc_ids) });
        }

        // Minimal response if no content
        if !has_content && tool_calls_acc.is_empty() {
            yield Ok(Event::default().event("response.created").data(serde_json::to_string(&json!({
                "type": "response.created",
                "response": {"id": &resp_id, "object": "response", "created_at": created,
                             "model": &model, "status": "in_progress", "output": [], "metadata": {}}
            })).unwrap()));
            yield Ok(Event::default().event("response.output_item.added").data(serde_json::to_string(&json!({
                "type": "response.output_item.added",
                "output_index": 0,
                "item": {"type": "message", "id": &msg_id, "status": "in_progress",
                         "role": "assistant", "content": []}
            })).unwrap()));
            yield Ok(Event::default().event("response.content_part.added").data(serde_json::to_string(&json!({
                "type": "response.content_part.added",
                "output_index": 0, "content_index": 0,
                "part": {"type": "output_text", "text": "", "annotations": []}
            })).unwrap()));
        }

        // Close text
        if has_content {
            yield Ok(Event::default().event("response.output_text.done").data(serde_json::to_string(&json!({
                "type": "response.output_text.done",
                "output_index": 0, "content_index": 0, "text": &full_text
            })).unwrap()));
            yield Ok(Event::default().event("response.output_item.done").data(serde_json::to_string(&json!({
                "type": "response.output_item.done",
                "output_index": 0,
                "item": {"type": "message", "id": &msg_id, "status": "completed",
                         "role": "assistant",
                         "content": [{"type": "output_text", "text": &full_text, "annotations": []}]}
            })).unwrap()));
        }

        // Tool call items
        let keys: Vec<_> = tool_calls_acc.keys().copied().collect();
        for &k in &keys {
            let tc = &tool_calls_acc[&k];
            let fc_id = rid("fc");
            let oi = 1 + k;
            yield Ok(Event::default().event("response.output_item.added").data(serde_json::to_string(&json!({
                "type": "response.output_item.added",
                "output_index": oi,
                "item": {"type": "function_call", "id": &fc_id, "call_id": tc["id"],
                         "name": tc["name"], "arguments": tc["arguments"], "status": "completed"}
            })).unwrap()));
            yield Ok(Event::default().event("response.output_item.done").data(serde_json::to_string(&json!({
                "type": "response.output_item.done",
                "output_index": oi,
                "item": {"type": "function_call", "id": &fc_id, "call_id": tc["id"],
                         "name": tc["name"], "arguments": tc["arguments"], "status": "completed"}
            })).unwrap()));
        }

        // Final output
        let mut final_output: Vec<Value> = Vec::new();
        if has_content {
            final_output.push(json!({
                "type": "message", "id": &msg_id, "status": "completed",
                "role": "assistant",
                "content": [{"type": "output_text", "text": &full_text, "annotations": []}],
            }));
        }
        for &k in &keys {
            let tc = &tool_calls_acc[&k];
            final_output.push(json!({
                "type": "function_call", "id": rid("fc"), "call_id": tc["id"],
                "name": tc["name"], "arguments": tc["arguments"], "status": "completed",
            }));
        }

        info!("COMPLETED text_len={} tools={} reasoning_len={}", full_text.len(), tool_calls_acc.len(), full_reasoning.len());

        let usage_in = final_usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let usage_out = final_usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

        yield Ok(Event::default().event("response.completed").data(serde_json::to_string(&json!({
            "type": "response.completed",
            "response": {
                "id": &resp_id, "object": "response", "created_at": created,
                "model": &model, "status": "completed", "output": final_output,
                "parallel_tool_calls": true,
                "usage": {"input_tokens": usage_in, "output_tokens": usage_out,
                         "total_tokens": usage_in + usage_out},
                "metadata": {},
            }
        })).unwrap()));
    };

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

// ---------------------------------------------------------------------------
// SSE Streaming for Anthropic upstream
// ---------------------------------------------------------------------------

async fn stream_anthropic(
    _state: ProxyState,
    cc: Value,
    headers: reqwest::header::HeaderMap,
    model: String,
    upstream_base: String,
) -> axum::response::Response {
    let resp_id = rid("resp");
    let msg_id = rid("msg");

    let stream = async_stream::stream! {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .unwrap_or_else(|_| Client::new());

        let resp = match http_client
            .post(format!("{}/v1/messages", upstream_base))
            .headers(headers)
            .json(&cc)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("ANTHROPIC STREAM UPSTREAM: {}", e);
                yield Ok::<_, axum::Error>(Event::default().event("error").data(format!("{}", e)));
                return;
            }
        };

        let http_status = resp.status();
        if !http_status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            error!("ANTHROPIC UPSTREAM {}: {}", http_status, &text[..text.len().min(500)]);
            yield Ok(Event::default().event("error").data(format!("Upstream {}: {}", http_status, &text[..text.len().min(500)])));
            yield Ok(Event::default().event("response.completed").data(serde_json::to_string(&json!({
                "type": "response.completed",
                "response": {
                    "id": &resp_id, "object": "response", "created_at": chrono::Utc::now().timestamp(),
                    "model": &model, "status": "failed", "output": [],
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0},
                    "metadata": {},
                }
            })).unwrap()));
            return;
        }

        let created = chrono::Utc::now().timestamp();
        let mut full_text = String::new();
        let mut tool_calls_acc: Vec<Value> = Vec::new();
        let mut has_content = false;
        let mut input_tokens = 0u64;
        let mut output_tokens = 0u64;

        let mut stream_resp = resp.bytes_stream();
        use futures::StreamExt;
        let mut buf = String::new();

        while let Some(chunk) = stream_resp.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(_) => break,
            };
            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = buf.find('\n') {
                let line = buf[..line_end].trim().to_string();
                buf = buf[line_end + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if line.starts_with("event: ") {
                    continue; // We handle data: lines, event type is implicit from structure
                }

                if !line.starts_with("data: ") {
                    continue;
                }

                let payload = &line[6..];
                let data: Value = match serde_json::from_str(payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let msg_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");

                match msg_type {
                    "message_start" => {
                        input_tokens = data.get("message")
                            .and_then(|m| m.get("usage"))
                            .and_then(|u| u.get("input_tokens"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                    }
                    "content_block_start" => {
                        if let Some(block) = data.get("content_block") {
                            match block.get("type").and_then(|v| v.as_str()) {
                                Some("text") => {
                                    if !has_content {
                                        has_content = true;
                                        yield Ok(Event::default().event("response.created").data(serde_json::to_string(&json!({
                                            "type": "response.created",
                                            "response": {"id": &resp_id, "object": "response", "created_at": created,
                                                         "model": &model, "status": "in_progress", "output": [], "metadata": {}}
                                        })).unwrap()));
                                        yield Ok(Event::default().event("response.output_item.added").data(serde_json::to_string(&json!({
                                            "type": "response.output_item.added",
                                            "output_index": 0,
                                            "item": {"type": "message", "id": &msg_id, "status": "in_progress",
                                                     "role": "assistant", "content": []}
                                        })).unwrap()));
                                        yield Ok(Event::default().event("response.content_part.added").data(serde_json::to_string(&json!({
                                            "type": "response.content_part.added",
                                            "output_index": 0, "content_index": 0,
                                            "part": {"type": "output_text", "text": "", "annotations": []}
                                        })).unwrap()));
                                    }
                                }
                                Some("tool_use") => {
                                    if !has_content {
                                        has_content = true;
                                        yield Ok(Event::default().event("response.created").data(serde_json::to_string(&json!({
                                            "type": "response.created",
                                            "response": {"id": &resp_id, "object": "response", "created_at": created,
                                                         "model": &model, "status": "in_progress", "output": [], "metadata": {}}
                                        })).unwrap()));
                                    }
                                    tool_calls_acc.push(json!({
                                        "id": block.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| rid("call")),
                                        "name": block.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                                        "arguments": "",
                                    }));
                                }
                                _ => {}
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(delta) = data.get("delta") {
                            match delta.get("type").and_then(|v| v.as_str()) {
                                Some("text_delta") => {
                                    let text = delta.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                    if !text.is_empty() {
                                        full_text.push_str(text);
                                        yield Ok(Event::default().event("response.output_text.delta").data(serde_json::to_string(&json!({
                                            "type": "response.output_text.delta",
                                            "output_index": 0, "content_index": 0, "delta": text
                                        })).unwrap()));
                                    }
                                }
                                Some("input_json_delta") => {
                                    if let Some(last) = tool_calls_acc.last_mut() {
                                        if let Some(json_str) = delta.get("partial_json").and_then(|v| v.as_str()) {
                                            let current = last["arguments"].as_str().unwrap_or("").to_string();
                                            last["arguments"] = json!(current + json_str);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "message_delta" => {
                        output_tokens = data.get("usage")
                            .and_then(|u| u.get("output_tokens"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                    }
                    _ => {}
                }
            }
        }

        // Close text
        if has_content && !full_text.is_empty() {
            yield Ok(Event::default().event("response.output_text.done").data(serde_json::to_string(&json!({
                "type": "response.output_text.done",
                "output_index": 0, "content_index": 0, "text": &full_text
            })).unwrap()));
            yield Ok(Event::default().event("response.output_item.done").data(serde_json::to_string(&json!({
                "type": "response.output_item.done",
                "output_index": 0,
                "item": {"type": "message", "id": &msg_id, "status": "completed",
                         "role": "assistant",
                         "content": [{"type": "output_text", "text": &full_text, "annotations": []}]}
            })).unwrap()));
        }

        // Tool call items
        for (idx, tc) in tool_calls_acc.iter().enumerate() {
            let fc_id = rid("fc");
            let oi = 1 + idx;
            yield Ok(Event::default().event("response.output_item.added").data(serde_json::to_string(&json!({
                "type": "response.output_item.added",
                "output_index": oi,
                "item": {"type": "function_call", "id": &fc_id, "call_id": tc["id"],
                         "name": tc["name"], "arguments": tc["arguments"], "status": "completed"}
            })).unwrap()));
            yield Ok(Event::default().event("response.output_item.done").data(serde_json::to_string(&json!({
                "type": "response.output_item.done",
                "output_index": oi,
                "item": {"type": "function_call", "id": &fc_id, "call_id": tc["id"],
                         "name": tc["name"], "arguments": tc["arguments"], "status": "completed"}
            })).unwrap()));
        }

        // Final output
        let mut final_output: Vec<Value> = Vec::new();
        if !full_text.is_empty() {
            final_output.push(json!({
                "type": "message", "id": &msg_id, "status": "completed",
                "role": "assistant",
                "content": [{"type": "output_text", "text": &full_text, "annotations": []}],
            }));
        }
        for tc in &tool_calls_acc {
            final_output.push(json!({
                "type": "function_call", "id": rid("fc"), "call_id": tc["id"],
                "name": tc["name"], "arguments": tc["arguments"], "status": "completed",
            }));
        }

        info!("ANTHROPIC COMPLETED text={} tools={}", full_text.len(), tool_calls_acc.len());

        yield Ok(Event::default().event("response.completed").data(serde_json::to_string(&json!({
            "type": "response.completed",
            "response": {
                "id": &resp_id, "object": "response", "created_at": created,
                "model": &model, "status": "completed", "output": final_output,
                "parallel_tool_calls": true,
                "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens,
                         "total_tokens": input_tokens + output_tokens},
                "metadata": {},
            }
        })).unwrap()));
    };

    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

// ---------------------------------------------------------------------------
// Model and health endpoints
// ---------------------------------------------------------------------------

pub async fn handle_models(State(state): State<ProxyState>) -> Json<Value> {
    let map = state.model_map.read().await;
    let data: Vec<Value> = map
        .keys()
        .map(|k| json!({"id": k, "object": "model", "owned_by": "codex-proxy", "slug": k}))
        .collect();
    Json(json!({"object": "list", "data": data}))
}

pub async fn handle_health(State(state): State<ProxyState>) -> Json<Value> {
    let upstream = state.upstream_base.read().await.clone();
    Json(json!({"status": "running", "upstream": upstream, "port": 9090}))
}

// ---------------------------------------------------------------------------
// Build router
// ---------------------------------------------------------------------------

pub fn build_router(state: ProxyState) -> Router {
    Router::new()
        .route("/v1/responses", post(handle_responses))
        .route("/responses", post(handle_responses))
        .route("/v1/models", get(handle_models))
        .route("/models", get(handle_models))
        .route("/health", get(handle_health))
        .with_state(state)
        .layer(
            tower_http::cors::CorsLayer::permissive()
        )
}
