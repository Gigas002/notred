//! `[events]` table deserialization (maps into `libnotred::EventsHooks`).

use libnotred::EventsHooks;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
pub struct EventsConfig {
    #[serde(default)]
    pub on_action: Option<Vec<String>>,
    #[serde(default)]
    pub on_button_left: Option<Vec<String>>,
    #[serde(default)]
    pub on_button_middle: Option<Vec<String>>,
    #[serde(default)]
    pub on_button_right: Option<Vec<String>>,
    #[serde(default)]
    pub on_touch: Option<Vec<String>>,
    #[serde(default)]
    pub on_notify: Option<Vec<String>>,
}

impl EventsConfig {
    pub fn to_hooks(&self) -> EventsHooks {
        EventsHooks {
            on_action: self.on_action.clone(),
            on_button_left: self.on_button_left.clone(),
            on_button_middle: self.on_button_middle.clone(),
            on_button_right: self.on_button_right.clone(),
            on_touch: self.on_touch.clone(),
            on_notify: self.on_notify.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
