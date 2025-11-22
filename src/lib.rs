use agent_stream_kit::ASKit;

pub mod application;
pub mod screen;

pub fn register_agents(askit: &ASKit) {
    application::register_agents(askit);
    screen::register_agents(askit);
}
