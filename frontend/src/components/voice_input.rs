//! Voice Input Component
//!
//! Provides voice-to-text input using the Web Audio API and AudioWorklet.
//! Audio is captured from the microphone, converted to PCM16 at 16kHz,
//! and sent via WebSocket to the backend for speech-to-text processing.

use gloo::utils::window;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioContextOptions, AudioWorkletNode, AudioWorkletNodeOptions, MediaStream,
    MediaStreamAudioSourceNode, MediaStreamConstraints, MessageEvent,
};
use yew::prelude::*;

/// Props for the VoiceInput component
#[derive(Properties, PartialEq)]
pub struct VoiceInputProps {
    /// Session ID to associate voice input with
    pub session_id: Uuid,
    /// Callback when recording state changes
    pub on_recording_change: Callback<bool>,
    /// Callback to send audio data (PCM16 bytes)
    pub on_audio_data: Callback<Vec<u8>>,
    /// Callback when an error occurs
    pub on_error: Callback<String>,
    /// Whether the component is disabled
    #[prop_or(false)]
    pub disabled: bool,
}

/// Voice input state
pub enum VoiceInputMsg {
    StartRecording,
    StopRecording,
    RecordingStarted(VoiceRecordingState),
    AudioData(Vec<u8>),
    Error(String),
}

/// State for active recording session
pub struct VoiceRecordingState {
    audio_context: AudioContext,
    worklet_node: AudioWorkletNode,
    source_node: MediaStreamAudioSourceNode,
    _media_stream: MediaStream,
}

impl Drop for VoiceRecordingState {
    fn drop(&mut self) {
        // Stop the worklet
        if let Ok(port) = self.worklet_node.port() {
            let _ = port.post_message(&JsValue::from_str(r#"{"command":"stop"}"#));
        }

        // Disconnect nodes
        self.source_node.disconnect().ok();
        self.worklet_node.disconnect().ok();

        // Close audio context
        let _ = self.audio_context.close();
    }
}

/// Voice input component with microphone button
pub struct VoiceInput {
    is_recording: bool,
    recording_state: Option<VoiceRecordingState>,
}

impl Component for VoiceInput {
    type Message = VoiceInputMsg;
    type Properties = VoiceInputProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            is_recording: false,
            recording_state: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            VoiceInputMsg::StartRecording => {
                if self.is_recording {
                    return false;
                }

                let link = ctx.link().clone();
                let on_audio = ctx.props().on_audio_data.clone();
                let on_error = ctx.props().on_error.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match start_recording(on_audio).await {
                        Ok(state) => {
                            link.send_message(VoiceInputMsg::RecordingStarted(state));
                        }
                        Err(e) => {
                            on_error.emit(e);
                            link.send_message(VoiceInputMsg::Error(
                                "Failed to start recording".to_string(),
                            ));
                        }
                    }
                });

                false
            }
            VoiceInputMsg::StopRecording => {
                if !self.is_recording {
                    return false;
                }

                // Drop the recording state to clean up
                self.recording_state = None;
                self.is_recording = false;
                ctx.props().on_recording_change.emit(false);
                true
            }
            VoiceInputMsg::RecordingStarted(state) => {
                self.recording_state = Some(state);
                self.is_recording = true;
                ctx.props().on_recording_change.emit(true);
                true
            }
            VoiceInputMsg::AudioData(data) => {
                ctx.props().on_audio_data.emit(data);
                false
            }
            VoiceInputMsg::Error(msg) => {
                log::error!("Voice input error: {}", msg);
                self.recording_state = None;
                self.is_recording = false;
                ctx.props().on_recording_change.emit(false);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let onclick = if self.is_recording {
            ctx.link().callback(|_| VoiceInputMsg::StopRecording)
        } else {
            ctx.link().callback(|_| VoiceInputMsg::StartRecording)
        };

        let disabled = ctx.props().disabled;
        let button_class = classes!(
            "voice-button",
            self.is_recording.then_some("recording"),
            disabled.then_some("disabled"),
        );

        let title = if self.is_recording {
            "Stop recording"
        } else {
            "Start voice input"
        };

        html! {
            <button
                class={button_class}
                onclick={onclick}
                disabled={disabled}
                title={title}
                type="button"
            >
                if self.is_recording {
                    <span class="voice-icon recording-icon">{ "\u{1F534}" }</span> // Red circle
                } else {
                    <span class="voice-icon mic-icon">{ "\u{1F3A4}" }</span> // Microphone
                }
            </button>
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        // Clean up recording state when component is destroyed
        self.recording_state = None;
    }
}

/// Start recording audio from the microphone
async fn start_recording(on_audio: Callback<Vec<u8>>) -> Result<VoiceRecordingState, String> {
    // Get user media (microphone)
    let navigator = window().navigator();
    let media_devices = navigator
        .media_devices()
        .map_err(|_| "Failed to get media devices")?;

    let constraints = MediaStreamConstraints::new();
    constraints.set_audio(&JsValue::TRUE);
    constraints.set_video(&JsValue::FALSE);

    let media_stream_promise = media_devices
        .get_user_media_with_constraints(&constraints)
        .map_err(|_| "Failed to request microphone access")?;

    let media_stream: MediaStream = JsFuture::from(media_stream_promise)
        .await
        .map_err(|e| format!("Microphone access denied: {:?}", e))?
        .dyn_into()
        .map_err(|_| "Invalid media stream")?;

    // Create audio context at 16kHz (matching Speech-to-Text requirement)
    let audio_options = AudioContextOptions::new();
    audio_options.set_sample_rate(16000.0);

    let audio_context = AudioContext::new_with_context_options(&audio_options)
        .map_err(|_| "Failed to create audio context")?;

    // Load the PCM processor worklet
    let worklet = audio_context
        .audio_worklet()
        .map_err(|_| "AudioWorklet not supported")?;

    JsFuture::from(
        worklet
            .add_module("/pcm-processor.js")
            .map_err(|_| "Failed to get module promise")?,
    )
    .await
    .map_err(|e| format!("Failed to load PCM processor: {:?}", e))?;

    // Create worklet node
    let worklet_options = AudioWorkletNodeOptions::new();
    let worklet_node =
        AudioWorkletNode::new_with_options(&audio_context, "pcm-processor", &worklet_options)
            .map_err(|_| "Failed to create worklet node")?;

    // Set up message handler for audio data from worklet
    let on_audio_clone = on_audio.clone();
    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
        if let Ok(data) = event.data().dyn_into::<js_sys::Object>() {
            if let Ok(audio_buffer) = js_sys::Reflect::get(&data, &JsValue::from_str("audioData")) {
                if let Ok(array_buffer) = audio_buffer.dyn_into::<js_sys::ArrayBuffer>() {
                    let uint8_array = js_sys::Uint8Array::new(&array_buffer);
                    let mut bytes = vec![0u8; uint8_array.length() as usize];
                    uint8_array.copy_to(&mut bytes);
                    on_audio_clone.emit(bytes);
                }
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    if let Ok(port) = worklet_node.port() {
        port.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    }
    onmessage.forget(); // Prevent closure from being dropped

    // Create source node from microphone stream
    let source_node = audio_context
        .create_media_stream_source(&media_stream)
        .map_err(|_| "Failed to create media stream source")?;

    // Connect: microphone -> worklet (worklet doesn't need to connect to destination)
    source_node
        .connect_with_audio_node(&worklet_node)
        .map_err(|_| "Failed to connect audio nodes")?;

    Ok(VoiceRecordingState {
        audio_context,
        worklet_node,
        source_node,
        _media_stream: media_stream,
    })
}
