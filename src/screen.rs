#![cfg(feature = "screen")]

use modular_agent_kit::photon_rs::{self, PhotonImage};
use modular_agent_kit::{
    Agent, AgentContext, AgentData, AgentError, AgentOutput, AgentSpec, AgentValue, AsAgent, MAK,
    async_trait, modular_agent,
};
use xcap::Monitor;

static CATEGORY: &str = "Lifelog";

static PORT_UNIT: &str = "unit";
static PORT_IMAGE: &str = "image";

static CONFIG_SCALE: &str = "scale";

#[modular_agent(
    title="Screen Capture",
    category=CATEGORY,
    inputs=[PORT_UNIT],
    outputs=[PORT_IMAGE],
    number_config(name=CONFIG_SCALE, default=1.0)
)]
struct ScreenCaptureAgent {
    data: AgentData,
}

impl ScreenCaptureAgent {
    async fn take_screenshot(&self) -> Result<PhotonImage, AgentError> {
        let monitors = Monitor::all()
            .map_err(|e| AgentError::InvalidValue(format!("Failed to get monitors: {}", e)))?;

        for monitor in monitors {
            // save only the primary monitor
            if monitor.is_primary().map_err(|e| {
                AgentError::InvalidValue(format!("Failed to check primary monitor: {}", e))
            })? {
                let image = monitor.capture_image().map_err(|e| {
                    AgentError::InvalidValue(format!("Failed to capture image: {}", e))
                })?;
                let width = image.width();
                let height = image.height();
                let image = PhotonImage::new(image.into_raw(), width, height);
                return Ok(image);
            }
        }
        Err(AgentError::Other("No primary monitor found".to_string()))
    }
}

#[async_trait]
impl AsAgent for ScreenCaptureAgent {
    fn new(mak: MAK, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(mak, id, spec),
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _port: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let mut screenshot = self.take_screenshot().await?;

        let scale = self.configs()?.get_number(CONFIG_SCALE)?;
        if scale != 1.0 {
            let width = screenshot.get_width();
            let height = screenshot.get_height();
            screenshot = photon_rs::transform::resize(
                &screenshot,
                (width as f64 * scale) as u32,
                (height as f64 * scale) as u32,
                photon_rs::transform::SamplingFilter::Nearest,
            );
        }

        let value = AgentValue::image(screenshot);
        self.output(ctx, PORT_IMAGE, value).await
    }
}
