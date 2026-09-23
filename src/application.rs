#![cfg(feature = "application")]

use active_win_pos_rs::{ActiveWindow, get_active_window};
use chrono::Utc;
use modular_agent_core::{
    AsModule, ModularAgent, Module, ModuleContext, ModuleData, ModuleOutput, ModuleSpec, Result,
    Value, async_trait, modular_agent,
};

static CATEGORY: &str = "Lifelog";

static PORT_UNIT: &str = "unit";
static PORT_EVENT: &str = "event";

static CONFIG_SKIP_UNCHANGED: &str = "skip_unchanged";
static CONFIG_IGNORE_NAMES: &str = "ignore_names";
static CONFIG_IGNORE_URLS: &str = "ignore_urls";

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
struct ActiveApplicationEvent {
    t: i64,
    name: String,
    title: String,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    text: String,
    url: Option<String>,
}

/// Reports which application window is in the foreground.
///
/// Each value on `unit` samples the active window once and emits an event describing it.
/// With `skip_unchanged` on, an event is emitted only when the window, its title, its
/// geometry, or the page URL differs from the previous sample. Applications listed in
/// `ignore_names` never produce events, and neither do pages whose URL contains one of
/// the `ignore_urls` entries.
///
/// When built with the `browser-url` feature, on Windows, and when the foreground window
/// belongs to a browser (Chrome, Edge, Firefox, Brave, Vivaldi, Opera, Arc), `url` holds
/// the URL of the page shown in the active tab. It is `null` for other applications, on
/// macOS and Linux, without the feature, and briefly right after a browser comes to the
/// front, before the browser exposes its page to accessibility clients.
///
/// # Ports
/// - Input `unit`: Trigger; the value is ignored
/// - Output `event`: Object with `t` (Unix time in milliseconds), `name` (application
///   name), `title` (window title, truncated to 250 characters), `x`, `y`, `width`,
///   `height` (window geometry in pixels), `text` (`name` and `title` joined with a
///   space), and `url` (page URL or `null`)
///
/// # Configuration
/// - `skip_unchanged`: Suppress the event when nothing changed since the last sample
///   (default: true)
/// - `ignore_names`: Application names, one per line, that never produce events
/// - `ignore_urls`: Substrings, one per line; a sample whose page URL contains any of
///   them produces no event
#[modular_agent(
    title="Active Application",
    category=CATEGORY,
    inputs=[PORT_UNIT],
    outputs=[PORT_EVENT],
    hint(height=2),
    boolean_config(name=CONFIG_SKIP_UNCHANGED, default=true),
    text_config(name=CONFIG_IGNORE_NAMES),
    text_config(name=CONFIG_IGNORE_URLS),
)]
struct ActiveApplicationModule {
    data: ModuleData,
    last_event: Option<ActiveApplicationEvent>,
}

impl ActiveApplicationModule {
    fn is_same(&mut self, app_event: &ActiveApplicationEvent) -> bool {
        if let Some(last_event) = &self.last_event {
            if app_event.x == last_event.x
                && app_event.y == last_event.y
                && app_event.width == last_event.width
                && app_event.height == last_event.height
                && app_event.text == last_event.text
                && app_event.url == last_event.url
            {
                return true;
            }
        }
        self.last_event = Some(app_event.clone());
        false
    }

    /// Samples the foreground window. The page URL is not read for applications in
    /// `ignore_names`, since the event is dropped anyway.
    async fn check_application(&self, ignore_names: &[&str]) -> Option<ActiveApplicationEvent> {
        const MAX_TITLE_LEN: usize = 250;

        let mut win = get_active_window().ok()?;
        if win.app_name.is_empty() {
            return None;
        }
        if win.title.chars().count() > MAX_TITLE_LEN {
            win.title = win.title.chars().take(MAX_TITLE_LEN).collect();
        };
        let url = if ignore_names.contains(&win.app_name.as_str()) {
            None
        } else {
            page_url(&win).await
        };

        let text = format!("{} {}", win.app_name, win.title).trim().to_string();
        Some(ActiveApplicationEvent {
            t: Utc::now().timestamp_millis(),
            name: win.app_name,
            title: win.title,
            x: win.position.x as i64,
            y: win.position.y as i64,
            width: win.position.width as i64,
            height: win.position.height as i64,
            text,
            url,
        })
    }
}

/// Reads the page URL when the active window belongs to a known browser.
#[cfg(feature = "browser-url")]
async fn page_url(win: &ActiveWindow) -> Option<String> {
    /// Executable stems (lowercase) of browsers whose page URL is read through UI Automation.
    const BROWSERS: &[&str] = &[
        "chrome", "msedge", "firefox", "brave", "vivaldi", "opera", "arc",
    ];
    const MAX_URL_LEN: usize = 2000;

    let exe = win
        .process_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if !BROWSERS.contains(&exe.as_str()) {
        return None;
    }
    tokio::task::spawn_blocking(browser_url)
        .await
        .ok()
        .flatten()
        .map(|url| url.chars().take(MAX_URL_LEN).collect())
}

#[cfg(not(feature = "browser-url"))]
async fn page_url(_win: &ActiveWindow) -> Option<String> {
    None
}

/// Reads the URL of the page shown in the foreground browser window.
///
/// Browsers expose the active tab's page as a UI Automation `Document` element whose
/// `ValuePattern` value is the page URL (Chromium and Firefox both do this). Blocks on
/// cross-process COM calls, so call it from a blocking thread.
#[cfg(all(feature = "browser-url", target_os = "windows"))]
fn browser_url() -> Option<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationValuePattern, TreeScope_Descendants,
        UIA_ControlTypePropertyId, UIA_DocumentControlTypeId, UIA_ValuePatternId,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    fn read(hwnd: HWND) -> windows::core::Result<String> {
        let document_type = VARIANT {
            Anonymous: VARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                    vt: VT_I4,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: VARIANT_0_0_0 {
                        lVal: UIA_DocumentControlTypeId.0,
                    },
                }),
            },
        };
        // SAFETY: plain COM calls on an in-process automation object; the VARIANT above is
        // a fully initialized VT_I4 that owns no heap memory.
        unsafe {
            // Initializing a thread more than once is fine; it just returns S_FALSE.
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            let root = uia.ElementFromHandle(hwnd)?;
            let condition =
                uia.CreatePropertyCondition(UIA_ControlTypePropertyId, &document_type)?;
            let document = root.FindFirst(TreeScope_Descendants, &condition)?;
            let value: IUIAutomationValuePattern =
                document.GetCurrentPatternAs(UIA_ValuePatternId)?;
            Ok(value.CurrentValue()?.to_string())
        }
    }

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    match read(hwnd) {
        Ok(url) if url.is_empty() || url == "about:blank" => None,
        Ok(url) => Some(url),
        Err(e) => {
            log::debug!("browser URL unavailable: {e}");
            None
        }
    }
}

#[cfg(all(feature = "browser-url", not(target_os = "windows")))]
fn browser_url() -> Option<String> {
    None
}

/// Non-empty trimmed lines of a list config. Blank lines are dropped, which matters for
/// `ignore_urls`: every URL contains "".
fn entries(list: &str) -> impl Iterator<Item = &str> {
    list.lines().map(str::trim).filter(|s| !s.is_empty())
}

#[async_trait]
impl AsModule for ActiveApplicationModule {
    fn new(ma: ModularAgent, id: String, spec: ModuleSpec) -> Result<Self> {
        Ok(Self {
            data: ModuleData::new(ma, id, spec),
            last_event: None,
        })
    }

    async fn process(&mut self, ctx: ModuleContext, _port: String, _value: Value) -> Result<()> {
        let ignore_names_text = self.configs()?.get_string_or_default(CONFIG_IGNORE_NAMES);
        let ignore_names: Vec<&str> = entries(&ignore_names_text).collect();
        let Some(app_event) = self.check_application(&ignore_names).await else {
            return Ok(());
        };

        let skip_unchanged = self.configs()?.get_bool_or_default(CONFIG_SKIP_UNCHANGED);
        if skip_unchanged && self.is_same(&app_event) {
            return Ok(());
        }

        if ignore_names.contains(&app_event.name.as_str()) {
            return Ok(());
        }

        let ignore_urls = self.configs()?.get_string_or_default(CONFIG_IGNORE_URLS);
        if let Some(url) = &app_event.url
            && entries(&ignore_urls).any(|p| url.contains(p))
        {
            return Ok(());
        }

        self.output(ctx, PORT_EVENT, Value::from_serialize(&app_event)?)
            .await
    }
}
