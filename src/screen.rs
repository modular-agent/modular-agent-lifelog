#![cfg(feature = "screen")]

use chrono::Utc;
use modular_agent_core::im::hashmap;
use modular_agent_core::photon_rs::{self, PhotonImage};
use modular_agent_core::{
    AsModule, Error, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec,
    Result, Value, async_trait, modular_agent,
};
use xcap::Monitor;

static CATEGORY: &str = "Lifelog";

static PORT_UNIT: &str = "unit";
static PORT_IMAGE: &str = "image";
static PORT_EVENT: &str = "event";

static CONFIG_SCALE: &str = "scale";
static CONFIG_INCLUDE_IMAGE: &str = "include_image";

#[modular_agent(
    title="Screen Capture",
    category=CATEGORY,
    inputs=[PORT_UNIT, PORT_EVENT],
    outputs=[PORT_IMAGE, PORT_EVENT],
    number_config(name=CONFIG_SCALE, default=1.0),
    boolean_config(name=CONFIG_INCLUDE_IMAGE),
)]
struct ScreenCaptureModule {
    data: ModuleData,
}

impl ScreenCaptureModule {
    // Returns (image, monitor_name, logical_x, logical_y)
    async fn take_screenshot(&self) -> Result<(PhotonImage, String, i64, i64)> {
        let monitors = Monitor::all()
            .map_err(|e| Error::InvalidValue(format!("Failed to get monitors: {}", e)))?;

        for monitor in monitors {
            // save only the primary monitor
            if monitor.is_primary().map_err(|e| {
                Error::InvalidValue(format!("Failed to check primary monitor: {}", e))
            })? {
                let monitor_name = monitor.name().unwrap_or_default();
                let mon_x = monitor.x().unwrap_or_default() as i64;
                let mon_y = monitor.y().unwrap_or_default() as i64;
                let image = monitor
                    .capture_image()
                    .map_err(|e| Error::InvalidValue(format!("Failed to capture image: {}", e)))?;
                let width = image.width();
                let height = image.height();
                let image = PhotonImage::new(image.into_raw(), width, height);
                return Ok((image, monitor_name, mon_x, mon_y));
            }
        }
        Err(Error::Other("No primary monitor found".to_string()))
    }

    // Returns Some((image, monitor_name)) or None if window is minimized/invalid.
    // x, y are absolute logical screen coordinates; width/height are logical pixel dimensions.
    async fn take_screenshot_region(
        &self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
    ) -> Result<Option<(PhotonImage, String)>> {
        if width <= 0 || height <= 0 {
            return Ok(None);
        }

        let monitor = Monitor::from_point(x as i32, y as i32).map_err(|e| {
            Error::InvalidValue(format!("Failed to find monitor at ({}, {}): {}", x, y, e))
        })?;
        let monitor_name = monitor.name().unwrap_or_default();

        let mon_x = monitor
            .x()
            .map_err(|e| Error::InvalidValue(format!("Monitor x: {}", e)))?
            as i64;
        let mon_y = monitor
            .y()
            .map_err(|e| Error::InvalidValue(format!("Monitor y: {}", e)))?
            as i64;
        let mon_w = monitor
            .width()
            .map_err(|e| Error::InvalidValue(format!("Monitor width: {}", e)))?
            as i64;
        let mon_h = monitor
            .height()
            .map_err(|e| Error::InvalidValue(format!("Monitor height: {}", e)))?
            as i64;

        // Convert absolute screen coords to monitor-relative logical coords, clamped to monitor bounds
        let rel_x = (x - mon_x).clamp(0, mon_w - 1);
        let rel_y = (y - mon_y).clamp(0, mon_h - 1);
        let w = width.clamp(1, mon_w - rel_x);
        let h = height.clamp(1, mon_h - rel_y);

        let image = monitor
            .capture_region(rel_x as u32, rel_y as u32, w as u32, h as u32)
            .map_err(|e| Error::InvalidValue(format!("Failed to capture region: {}", e)))?;

        let iw = image.width();
        let ih = image.height();
        let photo = PhotonImage::new(image.into_raw(), iw, ih);
        Ok(Some((photo, monitor_name)))
    }
}

#[async_trait]
impl AsModule for ScreenCaptureModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
        })
    }

    async fn process(&mut self, ctx: ModuleContext, port: String, value: Value) -> Result<()> {
        let scale = self.configs()?.get_number(CONFIG_SCALE)?;
        let include_image = self.configs()?.get_bool_or(CONFIG_INCLUDE_IMAGE, true);

        let (capture_x, capture_y, mut screenshot, monitor_name) = if port == PORT_EVENT {
            let x = value
                .get_i64("x")
                .ok_or_else(|| Error::InvalidValue("Missing 'x' in event".to_string()))?;
            let y = value
                .get_i64("y")
                .ok_or_else(|| Error::InvalidValue("Missing 'y' in event".to_string()))?;
            let w = value
                .get_i64("width")
                .ok_or_else(|| Error::InvalidValue("Missing 'width' in event".to_string()))?;
            let h = value
                .get_i64("height")
                .ok_or_else(|| Error::InvalidValue("Missing 'height' in event".to_string()))?;

            match self.take_screenshot_region(x, y, w, h).await? {
                Some((photo, name)) => (x, y, photo, name),
                None => return Ok(()), // minimized window — skip output
            }
        } else {
            let (photo, name, mon_x, mon_y) = self.take_screenshot().await?;
            (mon_x, mon_y, photo, name)
        };

        let image_width = screenshot.get_width();
        let image_height = screenshot.get_height();

        if scale != 1.0 {
            screenshot = photon_rs::transform::resize(
                &screenshot,
                (image_width as f64 * scale) as u32,
                (image_height as f64 * scale) as u32,
                photon_rs::transform::SamplingFilter::Nearest,
            );
        }

        let image = Value::image(screenshot);

        let mut event = hashmap! {
            "t".to_string() => Value::integer(Utc::now().timestamp_millis()),
            "monitor_name".to_string() => Value::string(&monitor_name),
            "x".to_string() => Value::integer(capture_x),
            "y".to_string() => Value::integer(capture_y),
            "width".to_string() => Value::integer(image_width as i64),
            "height".to_string() => Value::integer(image_height as i64),
            "scale".to_string() => Value::number(scale),
        };
        if include_image {
            event.insert("image".to_string(), image.clone());
        }

        self.output(ctx.clone(), PORT_EVENT, Value::object(event))
            .await?;
        self.output(ctx, PORT_IMAGE, image).await
    }
}
