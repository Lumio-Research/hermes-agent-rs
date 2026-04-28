//! Multimodal media workflow orchestrator
//!
//! Chains multiple AIGC tools into end-to-end content creation pipelines:
//! - Text → Image → Video (with optional audio overlay)
//! - Script → TTS → Video + Audio merge
//! - Image → Video → Audio → Final composite
//!
//! The orchestrator doesn't execute tools directly — it produces a structured
//! execution plan that the agent loop processes step by step, enabling
//! human-in-the-loop review at each stage.

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};

use hermes_core::{tool_schema, JsonSchema, ToolError, ToolHandler, ToolSchema};

use std::sync::Arc;

// ---------------------------------------------------------------------------
// MediaWorkflowBackend trait
// ---------------------------------------------------------------------------

/// Backend for orchestrating multimodal media workflows.
#[async_trait]
pub trait MediaWorkflowBackend: Send + Sync {
    /// Plan and optionally execute a media workflow.
    async fn execute_workflow(
        &self,
        workflow_type: &str,
        params: &Value,
    ) -> Result<String, ToolError>;
}

// ---------------------------------------------------------------------------
// Workflow types
// ---------------------------------------------------------------------------

/// Supported workflow templates.
#[derive(Debug, Clone, Copy)]
pub enum WorkflowType {
    /// Text prompt → GPT Image → SeedDance2 video
    TextToVideo,
    /// Text prompt → GPT Image → SeedDance2 video → TTS narration → merge
    TextToVideoWithNarration,
    /// Existing image → SeedDance2 video → optional TTS → merge
    ImageToVideo,
    /// Script text → TTS audio → GPT Image stills → SeedDance2 video → merge
    ScriptToPresentation,
    /// Custom: user defines the pipeline steps
    Custom,
}

impl WorkflowType {
    fn from_str(s: &str) -> Result<Self, ToolError> {
        match s {
            "text_to_video" => Ok(Self::TextToVideo),
            "text_to_video_narrated" => Ok(Self::TextToVideoWithNarration),
            "image_to_video" => Ok(Self::ImageToVideo),
            "script_to_presentation" => Ok(Self::ScriptToPresentation),
            "custom" => Ok(Self::Custom),
            _ => Err(ToolError::InvalidParams(format!(
                "Unknown workflow type: '{s}'. Use: text_to_video, text_to_video_narrated, \
                 image_to_video, script_to_presentation, custom"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// MediaWorkflowHandler
// ---------------------------------------------------------------------------

/// Tool for planning and executing multimodal media creation workflows.
pub struct MediaWorkflowHandler {
    backend: Arc<dyn MediaWorkflowBackend>,
}

impl MediaWorkflowHandler {
    pub fn new(backend: Arc<dyn MediaWorkflowBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl ToolHandler for MediaWorkflowHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let workflow_type = params
            .get("workflow")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'workflow' parameter".into()))?;

        // Validate workflow type
        let _ = WorkflowType::from_str(workflow_type)?;

        self.backend.execute_workflow(workflow_type, &params).await
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "workflow".into(),
            json!({
                "type": "string",
                "description": "Workflow template: 'text_to_video', 'text_to_video_narrated', 'image_to_video', 'script_to_presentation', 'custom'",
                "enum": ["text_to_video", "text_to_video_narrated", "image_to_video", "script_to_presentation", "custom"]
            }),
        );
        props.insert(
            "prompt".into(),
            json!({
                "type": "string",
                "description": "Main creative prompt / script for the workflow"
            }),
        );
        props.insert(
            "input_image".into(),
            json!({
                "type": "string",
                "description": "Path or URL to input image (for image_to_video workflow)"
            }),
        );
        props.insert(
            "narration_text".into(),
            json!({
                "type": "string",
                "description": "Text for TTS narration (auto-derived from prompt if omitted)"
            }),
        );
        props.insert(
            "voice".into(),
            json!({
                "type": "string",
                "description": "TTS voice to use for narration"
            }),
        );
        props.insert(
            "tts_provider".into(),
            json!({
                "type": "string",
                "description": "TTS provider: 'openai', 'elevenlabs', 'minimax', 'fish_audio'",
                "enum": ["openai", "elevenlabs", "minimax", "fish_audio"]
            }),
        );
        props.insert(
            "video_duration".into(),
            json!({
                "type": "number",
                "description": "Target video duration in seconds (default: 5.0)"
            }),
        );
        props.insert(
            "video_resolution".into(),
            json!({
                "type": "string",
                "description": "Video resolution: '720p', '1080p', '4k'",
                "default": "1080p"
            }),
        );
        props.insert(
            "aspect_ratio".into(),
            json!({
                "type": "string",
                "description": "Aspect ratio: '16:9', '9:16', '1:1'",
                "default": "16:9"
            }),
        );
        props.insert(
            "image_style".into(),
            json!({
                "type": "string",
                "description": "Style hint for image generation step"
            }),
        );
        props.insert(
            "steps".into(),
            json!({
                "type": "array",
                "description": "Custom pipeline steps (for 'custom' workflow type)",
                "items": {
                    "type": "object",
                    "properties": {
                        "tool": {"type": "string", "description": "Tool name to invoke"},
                        "params": {"type": "object", "description": "Parameters for the tool"},
                        "output_key": {"type": "string", "description": "Key to store output for later steps"}
                    }
                }
            }),
        );

        tool_schema(
            "media_workflow",
            "Plan and execute multimodal media creation workflows. Chains image generation (GPT Image), \
             video generation (SeedDance2), and audio synthesis (TTS) into end-to-end pipelines. \
             Returns a structured execution plan with step-by-step results.",
            JsonSchema::object(props, vec!["workflow".into()]),
        )
    }
}

// ---------------------------------------------------------------------------
// PlanningWorkflowBackend — generates execution plans
// ---------------------------------------------------------------------------

/// A workflow backend that generates structured execution plans for the
/// agent loop to process. Each step maps to a tool call the agent should
/// make, with outputs piped between steps.
pub struct PlanningWorkflowBackend;

impl PlanningWorkflowBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanningWorkflowBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MediaWorkflowBackend for PlanningWorkflowBackend {
    async fn execute_workflow(
        &self,
        workflow_type: &str,
        params: &Value,
    ) -> Result<String, ToolError> {
        let prompt = params.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
        let duration = params
            .get("video_duration")
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0);
        let resolution = params
            .get("video_resolution")
            .and_then(|v| v.as_str())
            .unwrap_or("1080p");
        let aspect_ratio = params
            .get("aspect_ratio")
            .and_then(|v| v.as_str())
            .unwrap_or("16:9");
        let voice = params.get("voice").and_then(|v| v.as_str());
        let tts_provider = params
            .get("tts_provider")
            .and_then(|v| v.as_str())
            .unwrap_or("openai");
        let narration = params
            .get("narration_text")
            .and_then(|v| v.as_str())
            .unwrap_or(prompt);
        let image_style = params.get("image_style").and_then(|v| v.as_str());
        let input_image = params.get("input_image").and_then(|v| v.as_str());

        let wf = WorkflowType::from_str(workflow_type)?;

        let plan = match wf {
            WorkflowType::TextToVideo => {
                let mut img_params = json!({
                    "prompt": prompt,
                    "size": "1536x1024",
                    "quality": "high",
                });
                if let Some(style) = image_style {
                    img_params
                        .as_object_mut()
                        .unwrap()
                        .insert("style".into(), json!(style));
                }

                json!({
                    "workflow": workflow_type,
                    "description": "Generate an image from text, then animate it into a video",
                    "steps": [
                        {
                            "step": 1,
                            "tool": "gpt_image_generate",
                            "description": "Generate a key frame image from the prompt",
                            "params": img_params,
                            "output_key": "generated_image"
                        },
                        {
                            "step": 2,
                            "tool": "video_generate",
                            "description": "Animate the generated image into a video",
                            "params": {
                                "prompt": prompt,
                                "input_image": "{{generated_image.images[0].file}}",
                                "duration": duration,
                                "resolution": resolution,
                                "aspect_ratio": aspect_ratio,
                            },
                            "output_key": "generated_video"
                        }
                    ],
                    "estimated_time_seconds": 60 + (duration * 12.0) as u64,
                })
            }

            WorkflowType::TextToVideoWithNarration => {
                json!({
                    "workflow": workflow_type,
                    "description": "Generate image, animate to video, add TTS narration",
                    "steps": [
                        {
                            "step": 1,
                            "tool": "gpt_image_generate",
                            "description": "Generate a key frame image",
                            "params": {
                                "prompt": prompt,
                                "size": "1536x1024",
                                "quality": "high",
                            },
                            "output_key": "generated_image"
                        },
                        {
                            "step": 2,
                            "tool": "text_to_speech",
                            "description": "Generate narration audio",
                            "params": {
                                "text": narration,
                                "voice": voice.unwrap_or("alloy"),
                                "provider": tts_provider,
                            },
                            "output_key": "narration_audio"
                        },
                        {
                            "step": 3,
                            "tool": "video_generate",
                            "description": "Animate the image into a video",
                            "params": {
                                "prompt": prompt,
                                "input_image": "{{generated_image.images[0].file}}",
                                "duration": duration,
                                "resolution": resolution,
                                "aspect_ratio": aspect_ratio,
                            },
                            "output_key": "generated_video"
                        },
                        {
                            "step": 4,
                            "tool": "terminal",
                            "description": "Merge video and audio with ffmpeg",
                            "params": {
                                "command": "ffmpeg -i {{generated_video.file}} -i {{narration_audio.file}} -c:v copy -c:a aac -shortest {{output_dir}}/final_output.mp4"
                            },
                            "output_key": "final_video"
                        }
                    ],
                    "estimated_time_seconds": 90 + (duration * 12.0) as u64,
                    "requirements": ["ffmpeg (for audio merge step)"],
                })
            }

            WorkflowType::ImageToVideo => {
                let img = input_image.ok_or_else(|| {
                    ToolError::InvalidParams(
                        "image_to_video workflow requires 'input_image' parameter".into(),
                    )
                })?;

                json!({
                    "workflow": workflow_type,
                    "description": "Animate an existing image into a video",
                    "steps": [
                        {
                            "step": 1,
                            "tool": "video_generate",
                            "description": "Animate the input image into a video",
                            "params": {
                                "prompt": prompt,
                                "input_image": img,
                                "duration": duration,
                                "resolution": resolution,
                                "aspect_ratio": aspect_ratio,
                            },
                            "output_key": "generated_video"
                        }
                    ],
                    "estimated_time_seconds": (duration * 12.0) as u64,
                })
            }

            WorkflowType::ScriptToPresentation => {
                json!({
                    "workflow": workflow_type,
                    "description": "Turn a script into a narrated video presentation",
                    "steps": [
                        {
                            "step": 1,
                            "tool": "text_to_speech",
                            "description": "Generate narration from the script",
                            "params": {
                                "text": narration,
                                "voice": voice.unwrap_or("nova"),
                                "provider": tts_provider,
                            },
                            "output_key": "narration_audio"
                        },
                        {
                            "step": 2,
                            "tool": "gpt_image_generate",
                            "description": "Generate a visual for the presentation",
                            "params": {
                                "prompt": format!("Professional presentation visual: {}", prompt),
                                "size": "1536x1024",
                                "quality": "high",
                            },
                            "output_key": "generated_image"
                        },
                        {
                            "step": 3,
                            "tool": "video_generate",
                            "description": "Animate the visual into a video",
                            "params": {
                                "prompt": format!("Smooth cinematic motion for: {}", prompt),
                                "input_image": "{{generated_image.images[0].file}}",
                                "duration": duration,
                                "resolution": resolution,
                                "aspect_ratio": aspect_ratio,
                            },
                            "output_key": "generated_video"
                        },
                        {
                            "step": 4,
                            "tool": "terminal",
                            "description": "Merge video and narration audio",
                            "params": {
                                "command": "ffmpeg -i {{generated_video.file}} -i {{narration_audio.file}} -c:v copy -c:a aac -shortest {{output_dir}}/presentation.mp4"
                            },
                            "output_key": "final_video"
                        }
                    ],
                    "estimated_time_seconds": 120 + (duration * 12.0) as u64,
                    "requirements": ["ffmpeg (for audio merge step)"],
                })
            }

            WorkflowType::Custom => {
                let steps = params.get("steps").ok_or_else(|| {
                    ToolError::InvalidParams("Custom workflow requires 'steps' array".into())
                })?;

                json!({
                    "workflow": "custom",
                    "description": "Custom media pipeline",
                    "steps": steps,
                    "note": "Execute each step in order, passing outputs between steps via output_key references."
                })
            }
        };

        Ok(plan.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_text_to_video_plan() {
        let handler = MediaWorkflowHandler::new(Arc::new(PlanningWorkflowBackend::new()));
        let result = handler
            .execute(json!({
                "workflow": "text_to_video",
                "prompt": "a sunset over the ocean"
            }))
            .await
            .unwrap();
        let plan: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(plan["steps"].as_array().unwrap().len(), 2);
        assert_eq!(plan["steps"][0]["tool"], "gpt_image_generate");
        assert_eq!(plan["steps"][1]["tool"], "video_generate");
    }

    #[tokio::test]
    async fn test_narrated_workflow_plan() {
        let handler = MediaWorkflowHandler::new(Arc::new(PlanningWorkflowBackend::new()));
        let result = handler
            .execute(json!({
                "workflow": "text_to_video_narrated",
                "prompt": "explaining quantum physics",
                "narration_text": "Quantum physics is the study of matter at the smallest scales.",
                "voice": "nova"
            }))
            .await
            .unwrap();
        let plan: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(plan["steps"].as_array().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn test_image_to_video_requires_image() {
        let handler = MediaWorkflowHandler::new(Arc::new(PlanningWorkflowBackend::new()));
        let result = handler
            .execute(json!({
                "workflow": "image_to_video",
                "prompt": "animate this"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_schema() {
        let handler = MediaWorkflowHandler::new(Arc::new(PlanningWorkflowBackend::new()));
        assert_eq!(handler.schema().name, "media_workflow");
    }
}
