#![cfg(feature = "screen")]

use chrono::Utc;
use modular_agent_core::im::hashmap;
use modular_agent_core::photon_rs::{self, PhotonImage};
use modular_agent_core::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent,
    ModularAgent, async_trait, modular_agent,
};
use xcap::Monitor;

static CATEGORY: &str = "Lifelog";

static PORT_UNIT: &str = "unit";
static PORT_IMAGE: &str = "image";
static PORT_EVENT: &str = "event";

static CONFIG_SCALE: &str = "scale";

#[modular_agent(
    title="Screen Capture",
    category=CATEGORY,
    inputs=[PORT_UNIT],
    outputs=[PORT_IMAGE, PORT_EVENT],
    number_config(name=CONFIG_SCALE, default=1.0)
)]
struct ScreenCaptureAgent {
    data: AgentData,
}

impl ScreenCaptureAgent {
    async fn take_screenshot(&self) -> Result<(PhotonImage, String), AgentError> {
        let monitors = Monitor::all()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to get monitors: {}", e)))?;

        for monitor in monitors {
            // save only the primary monitor
            if monitor.is_primary().map_err(|e| {
                AgentError::InvalidValue(format!("Failed to check primary monitor: {}", e))
            })? {
                let monitor_name = monitor.name().unwrap_or_default();
                let image = monitor.capture_image().map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to capture image: {}", e))
                })?;
                let width = image.width();
                let height = image.height();
                let image = PhotonImage::new(image.into_raw(), width, height);
                return Ok((image, monitor_name));
            }
        }
        Err(AgentError::Other("No primary monitor found".to_string()))
    }
}

#[async_trait]
impl AsAgent for ScreenCaptureAgent {
    fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(ma, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let (mut screenshot, monitor_name) = self.take_screenshot().await?;
        let original_width = screenshot.get_width();
        let original_height = screenshot.get_height();

        let scale = self.configs()?.get_number(CONFIG_SCALE)?;
        if scale != 1.0 {
            screenshot = photon_rs::transform::resize(
                &screenshot,
                (original_width as f64 * scale) as u32,
                (original_height as f64 * scale) as u32,
                photon_rs::transform::SamplingFilter::Nearest,
            );
        }

        let scaled_width = screenshot.get_width();
        let scaled_height = screenshot.get_height();
        let image = AgentValue::image(screenshot);

        let event = hashmap! {
            "t".to_string() => AgentValue::integer(Utc::now().timestamp_millis()),
            "monitor_name".to_string() => AgentValue::string(&monitor_name),
            "original_width".to_string() => AgentValue::integer(original_width as i64),
            "original_height".to_string() => AgentValue::integer(original_height as i64),
            "scaled_width".to_string() => AgentValue::integer(scaled_width as i64),
            "scaled_height".to_string() => AgentValue::integer(scaled_height as i64),
            "scale".to_string() => AgentValue::number(scale),
            "image".to_string() => image.clone(),
        };

        self.output(ctx.clone(), PORT_EVENT, AgentValue::object(event))
            .await?;
        self.output(ctx, PORT_IMAGE, image).await
    }
}
