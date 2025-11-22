use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentDefinition, AgentError, AgentOutput,
    AsAgent, AsAgentData, async_trait, new_agent_boxed,
};
// use base64::{self, Engine as _};
// use chrono::{DateTime, Utc};
// use image::{GrayImage, ImageFormat, RgbaImage};
use photon_rs::PhotonImage;
use xcap::Monitor;

// struct Screenshot {
//     timestamp: DateTime<Utc>,
//     monitor: u32,
//     image: RgbaImage,
// }

// #[derive(Clone, Debug, PartialEq, serde::Serialize)]
// struct ScreenEvent {
//     t: i64,
//     image: String,
//     image_id: String,
// }

// #[derive(Clone, Debug, PartialEq, serde::Serialize)]
// struct SameScreenEvent {
//     t: i64,
//     image_id: String,
// }

struct ScreenCaptureAgent {
    data: AsAgentData,
    // almost_black_threshold: u8,
    // non_blank_threshold: usize,
    // same_screen_ratio: f32,
    // last_image: Option<GrayImage>,
    // last_image_id: Option<String>,
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

    // fn is_blank(&self, image: &RgbaImage) -> bool {
    //     let mut count = 0;
    //     for pixel in image.pixels().step_by(120) {
    //         if pixel.0[0] >= self.almost_black_threshold
    //             || pixel.0[1] >= self.almost_black_threshold
    //             || pixel.0[2] >= self.almost_black_threshold
    //         {
    //             count += 1;
    //         }
    //         if count >= self.non_blank_threshold {
    //             return false;
    //         }
    //     }
    //     true
    // }

    // fn is_same(&mut self, screenshot: &Screenshot) -> bool {
    //     let gray_image = fast_downsample(&screenshot.image, 4);
    //     if let Some(last_image) = &self.last_image {
    //         let diff_ratio = get_difference_ratio2(&gray_image, last_image);
    //         if diff_ratio < self.same_screen_ratio {
    //             true
    //         } else {
    //             self.last_image = Some(gray_image);
    //             false
    //         }
    //     } else {
    //         self.last_image = Some(gray_image);
    //         false
    //     }
    // }
}

// fn rgba_to_base64_png(img: &RgbaImage) -> Result<String, AgentError> {
//     let mut buffer = Cursor::new(Vec::new());
//     img.write_to(&mut buffer, ImageFormat::Png)
//         .map_err(|e| AgentError::InvalidValue(format!("Failed to write image to buffer: {}", e)))?;
//     Ok(base64::engine::general_purpose::STANDARD.encode(buffer.into_inner()))
// }

// fn fast_downsample(img: &RgbaImage, scale: u32) -> GrayImage {
//     let new_width = img.width() / scale;
//     let new_height = img.height() / scale;
//     let scale_squared = (scale * scale) as u32;

//     let mut result = GrayImage::new(new_width, new_height);

//     for y in 0..new_height {
//         for x in 0..new_width {
//             let mut sum = 0u32;

//             for dy in 0..scale {
//                 for dx in 0..scale {
//                     let px = img.get_pixel(x * scale + dx, y * scale + dy);
//                     // RGBA to Grayscale
//                     sum += (px[0] as u32 * 299 + px[1] as u32 * 587 + px[2] as u32 * 114) / 1000;
//                 }
//             }
//             result.put_pixel(x, y, image::Luma([(sum / scale_squared) as u8]));
//         }
//     }

//     result
// }

// fn get_difference_ratio2(img1: &GrayImage, img2: &GrayImage) -> f32 {
//     if img1.dimensions() != img2.dimensions() {
//         return 1.0;
//     }
//     let different_pixels = img1
//         .pixels()
//         .zip(img2.pixels())
//         .filter(|(p1, p2)| {
//             let diff = if p1.0[0] > p2.0[0] {
//                 p1.0[0] - p2.0[0]
//             } else {
//                 p2.0[0] - p1.0[0]
//             };
//             diff > 5 // TODO: setting
//         })
//         .count();
//     different_pixels as f32 / (img1.width() * img1.height()) as f32
// }

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
            // last_image: None,
            // last_image_id: None,
            // almost_black_threshold: 20,
            // non_blank_threshold: 400,
            // same_screen_ratio: 0.01,
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    // fn configs_changed(&mut self) -> Result<(), AgentError> {
    //     self.almost_black_threshold =
    //         self.configs()?.get_integer(CONFIG_ALMOST_BLACK_THRESHOLD)? as u8;
    //     self.non_blank_threshold =
    //         self.configs()?.get_integer(CONFIG_NON_BLANK_THRESHOLD)? as usize;
    //     self.same_screen_ratio = self.configs()?.get_number(CONFIG_SAME_SCREEN_RATIO)? as f32;
    //     Ok(())
    // }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _data: AgentData,
    ) -> Result<(), AgentError> {
        let mut screenshot = self.take_screenshot().await?;
        // if screenshot.is_none() {
        //     return Ok(());
        // }
        // let screenshot = screenshot.unwrap();

        // let same = self.is_same(&screenshot);

        // if same {
        //     let ts = screenshot.timestamp;
        //     let screen_event = SameScreenEvent {
        //         t: ts.timestamp_millis(),
        //         image_id: self.last_image_id.clone().unwrap(),
        //     };
        //     let data = AgentData::from_serialize(&screen_event)?;
        //     self.try_output(ctx, PIN_IMAGE, data)?;
        //     return Ok(());
        // }

        // convert screenshot image into base64 string

        // let ts = screenshot.timestamp;
        // let ymd = ts.format("%Y%m%d").to_string();
        // let hms = ts.format("%H%M%S").to_string();
        // let image = rgba_to_base64_png(&screenshot.image)?;
        // let image_id = format!("{}-{}-{}", ymd, hms, screenshot.monitor);

        // let screen_event = ScreenEvent {
        //     t: ts.timestamp_millis(),
        //     image,
        //     image_id: image_id.clone(),
        // };

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

        // self.last_image_id = Some(image_id);

        Ok(())
    }
}

static AGENT_KIND: &str = "agent";
static CATEGORY: &str = "Lifelogging";

static PIN_UNIT: &str = "unit";
static PIN_IMAGE: &str = "image";

// static CONFIG_ALMOST_BLACK_THRESHOLD: &str = "almost_black_threshold";
// static CONFIG_NON_BLANK_THRESHOLD: &str = "non_blank_threshold";
// static CONFIG_SAME_SCREEN_RATIO: &str = "same_screen_ratio";
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
        // .with_default_configs(vec![
        //     (
        //         CONFIG_ALMOST_BLACK_THRESHOLD,
        //         AgentConfigEntry::new(20, "integer"),
        //     ),
        //     (
        //         CONFIG_NON_BLANK_THRESHOLD,
        //         AgentConfigEntry::new(400, "integer"),
        //     ),
        //     (
        //         CONFIG_SAME_SCREEN_RATIO,
        //         AgentConfigEntry::new(0.01, "number"),
        //     ),
        // ]),
    );
}
