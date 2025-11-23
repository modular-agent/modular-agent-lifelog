use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AsAgent, AsAgentData, async_trait, new_agent_boxed,
};
use photon_rs::PhotonImage;
use xcap::Monitor;

struct ScreenCaptureAgent {
    data: AsAgentData,
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
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, config),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _data: AgentData,
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

        let data = AgentData::image(screenshot);
        self.try_output(ctx, PIN_IMAGE, data)?;

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Lifelogging";

static PIN_UNIT: &str = "unit";
static PIN_IMAGE: &str = "image";

static CONFIG_SCALE: &str = "scale";

pub fn register_agents(askit: &ASKit) {
    askit.register_agent(
        AgentDefinition::new(
            AGENT_KIND,
            "lifelogging_screen_capture",
            Some(new_agent_boxed::<ScreenCaptureAgent>),
        )
        .title("Screen Capture")
        .category(CATEGORY)
        .inputs(vec![PIN_UNIT])
        .outputs(vec![PIN_IMAGE])
        .number_config(CONFIG_SCALE, 1.0),
    );
}
