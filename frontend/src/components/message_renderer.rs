use serde::{Deserialize, Serialize};
use serde_json::Value;
use yew::prelude::*;

/// Parsed message types from Claude Code
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeMessage {
    #[serde(rename = "system")]
    System(SystemMessage),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "result")]
    Result(ResultMessage),
    #[serde(rename = "user")]
    User(UserMessage),
    #[serde(rename = "error")]
    Error(ErrorMessage),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserMessage {
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorMessage {
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemMessage {
    pub subtype: Option<String>,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub claude_code_version: Option<String>,
    pub tools: Option<Vec<String>>,
    pub agents: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
    pub slash_commands: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<Value>>,
    pub plugins: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssistantMessage {
    pub message: Option<MessageContent>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageContent {
    pub id: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
    pub content: Option<Vec<ContentBlock>>,
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageInfo {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResultMessage {
    pub subtype: Option<String>,
    pub session_id: Option<String>,
    pub result: Option<String>,
    pub is_error: Option<bool>,
    pub duration_ms: Option<u64>,
    pub duration_api_ms: Option<u64>,
    pub total_cost_usd: Option<f64>,
    pub num_turns: Option<u64>,
    pub usage: Option<UsageInfo>,
}

#[derive(Properties, PartialEq)]
pub struct MessageRendererProps {
    pub json: String,
}

#[function_component(MessageRenderer)]
pub fn message_renderer(props: &MessageRendererProps) -> Html {
    // Try to parse as a known message type
    let parsed: Result<ClaudeMessage, _> = serde_json::from_str(&props.json);

    match parsed {
        Ok(ClaudeMessage::System(msg)) => render_system_message(&msg),
        Ok(ClaudeMessage::Assistant(msg)) => render_assistant_message(&msg),
        Ok(ClaudeMessage::Result(msg)) => render_result_message(&msg),
        Ok(ClaudeMessage::User(msg)) => render_user_message(&msg),
        Ok(ClaudeMessage::Error(msg)) => render_error_message(&msg),
        Ok(ClaudeMessage::Unknown) | Err(_) => render_raw_json(&props.json),
    }
}

fn render_user_message(msg: &UserMessage) -> Html {
    let content = msg.content.as_deref().unwrap_or("");

    html! {
        <div class="claude-message user-message">
            <div class="message-header">
                <span class="message-type-badge user">{ "You" }</span>
            </div>
            <div class="message-body">
                <div class="user-text">{ content }</div>
            </div>
        </div>
    }
}

fn render_error_message(msg: &ErrorMessage) -> Html {
    let message = msg.message.as_deref().unwrap_or("Unknown error");

    html! {
        <div class="claude-message error-message-display">
            <div class="message-header">
                <span class="message-type-badge result error">{ "Error" }</span>
            </div>
            <div class="message-body">
                <div class="error-text">{ message }</div>
            </div>
        </div>
    }
}

fn render_system_message(msg: &SystemMessage) -> Html {
    let subtype = msg.subtype.as_deref().unwrap_or("system");

    html! {
        <div class="claude-message system-message">
            <div class="message-header">
                <span class="message-type-badge system">{ "System" }</span>
                <span class="message-subtype">{ subtype }</span>
            </div>
            <div class="message-body">
                {
                    if subtype == "init" {
                        html! {
                            <div class="init-info">
                                <div class="init-main">
                                    {
                                        if let Some(model) = &msg.model {
                                            html! {
                                                <div class="init-row">
                                                    <span class="init-label">{ "Model" }</span>
                                                    <span class="init-value model">{ model }</span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if let Some(cwd) = &msg.cwd {
                                            html! {
                                                <div class="init-row">
                                                    <span class="init-label">{ "Directory" }</span>
                                                    <span class="init-value path">{ cwd }</span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if let Some(version) = &msg.claude_code_version {
                                            html! {
                                                <div class="init-row">
                                                    <span class="init-label">{ "Version" }</span>
                                                    <span class="init-value">{ version }</span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                                <div class="init-details">
                                    {
                                        if let Some(tools) = &msg.tools {
                                            html! {
                                                <div class="detail-group" title={tools.join(", ")}>
                                                    <span class="detail-count">{ tools.len() }</span>
                                                    <span class="detail-label">{ "tools" }</span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if let Some(agents) = &msg.agents {
                                            html! {
                                                <div class="detail-group" title={agents.join(", ")}>
                                                    <span class="detail-count">{ agents.len() }</span>
                                                    <span class="detail-label">{ "agents" }</span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if let Some(commands) = &msg.slash_commands {
                                            if !commands.is_empty() {
                                                html! {
                                                    <div class="detail-group" title={commands.iter().map(|c| format!("/{}", c)).collect::<Vec<_>>().join(", ")}>
                                                        <span class="detail-count">{ commands.len() }</span>
                                                        <span class="detail-label">{ "commands" }</span>
                                                    </div>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            </div>
                        }
                    } else {
                        html! { <span class="muted">{ "System event" }</span> }
                    }
                }
            </div>
        </div>
    }
}

fn render_assistant_message(msg: &AssistantMessage) -> Html {
    let content_text = msg
        .message
        .as_ref()
        .and_then(|m| m.content.as_ref())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    ContentBlock::Other => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    let usage = msg.message.as_ref().and_then(|m| m.usage.as_ref());
    let model = msg
        .message
        .as_ref()
        .and_then(|m| m.model.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("");

    let usage_tooltip = usage
        .map(|u| {
            format!(
                "Input: {} | Output: {} | Cache read: {} | Cache created: {}",
                u.input_tokens.unwrap_or(0),
                u.output_tokens.unwrap_or(0),
                u.cache_read_input_tokens.unwrap_or(0),
                u.cache_creation_input_tokens.unwrap_or(0)
            )
        })
        .unwrap_or_default();

    html! {
        <div class="claude-message assistant-message">
            <div class="message-header">
                <span class="message-type-badge assistant">{ "Assistant" }</span>
                {
                    if !model.is_empty() {
                        html! { <span class="model-name" title={model.to_string()}>{ shorten_model_name(model) }</span> }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(u) = usage {
                        html! {
                            <span class="usage-badge" title={usage_tooltip}>
                                <span class="token-count">{ format!("{}", u.output_tokens.unwrap_or(0)) }</span>
                                <span class="token-label">{ "tokens" }</span>
                            </span>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
            <div class="message-body">
                <div class="assistant-text">{ content_text }</div>
            </div>
        </div>
    }
}

fn render_result_message(msg: &ResultMessage) -> Html {
    let is_error = msg.is_error.unwrap_or(false);
    let status_class = if is_error { "error" } else { "success" };

    let duration_ms = msg.duration_ms.unwrap_or(0);
    let api_ms = msg.duration_api_ms.unwrap_or(0);
    let cost = msg.total_cost_usd.unwrap_or(0.0);
    let turns = msg.num_turns.unwrap_or(0);

    let timing_tooltip = format!(
        "Total: {}ms | API: {}ms | Turns: {}",
        duration_ms, api_ms, turns
    );

    // Result message is just a compact stats bar - the assistant message already showed the content
    html! {
        <div class={classes!("claude-message", "result-message", status_class)}>
            <div class="result-stats-bar">
                <span class={classes!("result-status", status_class)}>
                    { if is_error { "✗" } else { "✓" } }
                </span>
                <span class="stat-item duration" title={timing_tooltip.clone()}>
                    { format_duration(duration_ms) }
                </span>
                {
                    if cost > 0.0 {
                        html! {
                            <span class="stat-item cost" title={format!("${:.6}", cost)}>
                                { format_cost(cost) }
                            </span>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(usage) = &msg.usage {
                        html! {
                            <>
                                <span class="stat-item tokens" title="Input tokens">
                                    { format!("{}↓", usage.input_tokens.unwrap_or(0)) }
                                </span>
                                <span class="stat-item tokens" title="Output tokens">
                                    { format!("{}↑", usage.output_tokens.unwrap_or(0)) }
                                </span>
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if turns > 1 {
                        html! {
                            <span class="stat-item turns" title="API turns">
                                { format!("{} turns", turns) }
                            </span>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
    }
}

fn render_raw_json(json: &str) -> Html {
    // Try to pretty-print, otherwise show as-is
    let display = serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| json.to_string());

    html! {
        <div class="claude-message raw-message">
            <div class="message-header">
                <span class="message-type-badge raw">{ "Raw" }</span>
            </div>
            <div class="message-body">
                <pre class="raw-json">{ display }</pre>
            </div>
        </div>
    }
}

fn shorten_model_name(model: &str) -> String {
    if model.contains("opus") {
        "Opus".to_string()
    } else if model.contains("sonnet") {
        "Sonnet".to_string()
    } else if model.contains("haiku") {
        "Haiku".to_string()
    } else {
        model.split('-').next().unwrap_or(model).to_string()
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60000;
        let secs = (ms % 60000) / 1000;
        format!("{}m {}s", mins, secs)
    }
}

fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else if cost < 1.0 {
        format!("${:.3}", cost)
    } else {
        format!("${:.2}", cost)
    }
}
