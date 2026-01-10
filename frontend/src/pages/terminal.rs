use crate::components::MessageRenderer;
use crate::utils;
use futures_util::{SinkExt, StreamExt};
use gloo_net::websocket::{futures::WebSocket, Message};
use shared::ProxyMessage;
use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TerminalPageProps {
    pub session_id: String,
}

pub enum TerminalMsg {
    SendInput,
    UpdateInput(String),
    ReceivedOutput(String),
    WebSocketConnected(Rc<RefCell<Option<futures_util::stream::SplitSink<WebSocket, Message>>>>),
    WebSocketError(String),
}

pub struct TerminalPage {
    messages: Vec<String>, // Raw JSON messages for rendering
    input_value: String,
    ws_connected: bool,
    ws_sender: Option<Rc<RefCell<Option<futures_util::stream::SplitSink<WebSocket, Message>>>>>,
}

impl Component for TerminalPage {
    type Message = TerminalMsg;
    type Properties = TerminalPageProps;

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link().clone();
        let session_id = ctx.props().session_id.clone();

        spawn_local(async move {
            let ws_endpoint = utils::ws_url("/ws/client");
            match WebSocket::open(&ws_endpoint) {
                Ok(ws) => {
                    let (mut sender, mut receiver) = ws.split();

                    // Parse session_id as UUID
                    let session_uuid = match Uuid::parse_str(&session_id) {
                        Ok(uuid) => uuid,
                        Err(e) => {
                            link.send_message(TerminalMsg::WebSocketError(
                                format!("Invalid session ID: {}", e),
                            ));
                            return;
                        }
                    };

                    // Send registration message with the session we want to connect to
                    let register_msg = ProxyMessage::Register {
                        session_id: session_uuid,
                        session_name: session_id.clone(), // Use the string for display purposes
                        auth_token: None,
                        working_directory: String::new(),
                        resuming: false, // Web clients don't "resume" in the same sense
                    };

                    if let Ok(json) = serde_json::to_string(&register_msg) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            link.send_message(TerminalMsg::WebSocketError(
                                "Failed to send registration".to_string(),
                            ));
                            return;
                        }
                    }

                    // Wrap sender in Rc<RefCell> so we can share it
                    let sender = Rc::new(RefCell::new(Some(sender)));
                    link.send_message(TerminalMsg::WebSocketConnected(sender));

                    // Listen for messages
                    while let Some(msg) = receiver.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Ok(proxy_msg) = serde_json::from_str::<ProxyMessage>(&text) {
                                    match proxy_msg {
                                        ProxyMessage::ClaudeOutput { content } => {
                                            // Send the raw JSON content for rich rendering
                                            link.send_message(TerminalMsg::ReceivedOutput(
                                                content.to_string(),
                                            ));
                                        }
                                        ProxyMessage::Error { message } => {
                                            // Create an error JSON object
                                            let error_json = serde_json::json!({
                                                "type": "error",
                                                "message": message
                                            });
                                            link.send_message(TerminalMsg::ReceivedOutput(
                                                error_json.to_string(),
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("WebSocket error: {:?}", e);
                                link.send_message(TerminalMsg::WebSocketError(format!("{:?}", e)));
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to connect WebSocket: {:?}", e);
                    link.send_message(TerminalMsg::WebSocketError(format!("{:?}", e)));
                }
            }
        });

        Self {
            messages: vec![],
            input_value: String::new(),
            ws_connected: false,
            ws_sender: None,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            TerminalMsg::UpdateInput(value) => {
                self.input_value = value;
                true
            }
            TerminalMsg::SendInput => {
                let input = self.input_value.trim().to_string();
                if input.is_empty() {
                    return false;
                }

                // Create a user message JSON for display
                let user_msg = serde_json::json!({
                    "type": "user",
                    "content": input
                });
                self.messages.push(user_msg.to_string());
                self.input_value.clear();

                // Send to WebSocket
                if let Some(ref sender_rc) = self.ws_sender {
                    let sender_rc = sender_rc.clone();
                    let msg = ProxyMessage::ClaudeInput {
                        content: serde_json::Value::String(input),
                    };

                    spawn_local(async move {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if let Some(ref mut sender) = *sender_rc.borrow_mut() {
                                let _ = sender.send(Message::Text(json)).await;
                            }
                        }
                    });
                }
                true
            }
            TerminalMsg::ReceivedOutput(output) => {
                self.messages.push(output);
                true
            }
            TerminalMsg::WebSocketConnected(sender) => {
                self.ws_connected = true;
                self.ws_sender = Some(sender);
                true
            }
            TerminalMsg::WebSocketError(err) => {
                let error_msg = serde_json::json!({
                    "type": "error",
                    "message": format!("Connection error: {}", err)
                });
                self.messages.push(error_msg.to_string());
                self.ws_connected = false;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        let handle_submit = link.callback(|e: SubmitEvent| {
            e.prevent_default();
            TerminalMsg::SendInput
        });

        let handle_input = link.callback(|e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            TerminalMsg::UpdateInput(input.value())
        });

        let handle_back = Callback::from(|_| {
            if let Some(window) = web_sys::window() {
                let _ = window.history().map(|h| h.back());
            }
        });

        html! {
            <div class="terminal-page">
                <header class="terminal-header">
                    <button class="back-button" onclick={handle_back}>
                        { "← Back to Dashboard" }
                    </button>
                    <div class="session-info">
                        <span class="session-id">{ "Session: " }{ &ctx.props().session_id }</span>
                        <span class={if self.ws_connected { "status connected" } else { "status disconnected" }}>
                            { if self.ws_connected { "● Connected" } else { "○ Disconnected" } }
                        </span>
                    </div>
                </header>

                <div class="terminal-content">
                    <div class="messages">
                        {
                            self.messages.iter().map(|json| {
                                html! {
                                    <MessageRenderer json={json.clone()} />
                                }
                            }).collect::<Html>()
                        }
                    </div>

                    <form class="input-form" onsubmit={handle_submit}>
                        <input
                            type="text"
                            class="message-input"
                            placeholder="Type your message to Claude..."
                            value={self.input_value.clone()}
                            oninput={handle_input}
                            disabled={!self.ws_connected}
                        />
                        <button type="submit" class="send-button" disabled={!self.ws_connected}>
                            { "Send" }
                        </button>
                    </form>
                </div>
            </div>
        }
    }
}
