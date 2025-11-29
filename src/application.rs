use active_win_pos_rs::get_active_window;
use agent_stream_kit::{
    ASKit, Agent, AgentConfigs, AgentContext, AgentData, AgentError, AgentOutput, AgentValue,
    AsAgent, async_trait,
};
use askit_macros::askit_agent;
use chrono::Utc;

static CATEGORY: &str = "Lifelog";

static PIN_UNIT: &str = "unit";
static PIN_EVENT: &str = "event";

static CONFIG_SKIP_UNCHANGED: &str = "skip_unchanged";
static CONFIG_IGNORE_LIST: &str = "ignore_list";

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
}

#[askit_agent(
    title="Active Application",
    category=CATEGORY,
    inputs=[PIN_UNIT],
    outputs=[PIN_EVENT],
    boolean_config(name=CONFIG_SKIP_UNCHANGED, default=true),
    string_config(name=CONFIG_IGNORE_LIST),
)]
struct ActiveApplicationAgent {
    data: AgentData,
    last_event: Option<ActiveApplicationEvent>,
}

impl ActiveApplicationAgent {
    fn is_same(&mut self, app_event: &ActiveApplicationEvent) -> bool {
        if let Some(last_event) = &self.last_event {
            if app_event.x == last_event.x
                && app_event.y == last_event.y
                && app_event.width == last_event.width
                && app_event.height == last_event.height
                && app_event.text == last_event.text
            {
                return true;
            }
        }
        self.last_event = Some(app_event.clone());
        false
    }

    async fn check_application(&self) -> Option<ActiveApplicationEvent> {
        const MAX_TITLE_LEN: usize = 250;

        match get_active_window() {
            Ok(mut win) => {
                // sanitize app_name and title
                if win.app_name.is_empty() {
                    return None;
                }
                if win.title.chars().count() > MAX_TITLE_LEN {
                    win.title = win.title.chars().take(MAX_TITLE_LEN).collect();
                };

                let text = format!("{} {}", win.app_name, win.title).trim().to_string();
                let info = ActiveApplicationEvent {
                    t: Utc::now().timestamp_millis(),
                    // process_id: win.process_id as i64,
                    // path: path,
                    name: win.app_name,
                    title: win.title,
                    x: win.position.x as i64,
                    y: win.position.y as i64,
                    width: win.position.width as i64,
                    height: win.position.height as i64,
                    text: text,
                };
                Some(info)
            }
            _ => None,
        }
    }
}

#[async_trait]
impl AsAgent for ActiveApplicationAgent {
    fn new(
        askit: ASKit,
        id: String,
        def_name: String,
        config: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AgentData::new(askit, id, def_name, config),
            last_event: None,
        })
    }

    async fn process(
        &mut self,
        ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        let Some(app_event) = self.check_application().await else {
            return Ok(());
        };

        let skip_unchanged = self.configs()?.get_bool_or_default(CONFIG_SKIP_UNCHANGED);
        if skip_unchanged && self.is_same(&app_event) {
            return Ok(());
        }

        let ignore_list = self.configs()?.get_string_or_default(CONFIG_IGNORE_LIST);
        let ignore_vec: Vec<&str> = ignore_list.split(',').map(|s| s.trim()).collect();
        if ignore_vec.contains(&app_event.name.as_str()) {
            return Ok(());
        }

        self.try_output(ctx, PIN_EVENT, AgentValue::from_serialize(&app_event)?)
    }
}
