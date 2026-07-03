//! Resolved `[events]` hooks and poshanka-parity override merge.

use crate::wire::Urgency;

/// Merged event hooks for a notification context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventsHooks {
    pub on_action: Option<HookArgv>,
    pub on_button_left: Option<HookArgv>,
    pub on_button_middle: Option<HookArgv>,
    pub on_button_right: Option<HookArgv>,
    pub on_touch: Option<HookArgv>,
    pub on_notify: Option<HookArgv>,
}

/// Shell argv for one event hook (`[events].on_*` in config).
pub type HookArgv = Vec<String>;

/// Subscriber-reported pointer gesture (`notredctl input`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    ButtonLeft,
    ButtonMiddle,
    ButtonRight,
    Touch,
}

impl EventKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "button_left" => Some(Self::ButtonLeft),
            "button_middle" => Some(Self::ButtonMiddle),
            "button_right" => Some(Self::ButtonRight),
            "touch" => Some(Self::Touch),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ButtonLeft => "button_left",
            Self::ButtonMiddle => "button_middle",
            Self::ButtonRight => "button_right",
            Self::Touch => "touch",
        }
    }

    pub fn hook<'a>(&self, hooks: &'a EventsHooks) -> Option<&'a HookArgv> {
        match self {
            Self::ButtonLeft => hooks.on_button_left.as_ref(),
            Self::ButtonMiddle => hooks.on_button_middle.as_ref(),
            Self::ButtonRight => hooks.on_button_right.as_ref(),
            Self::Touch => hooks.on_touch.as_ref(),
        }
    }
}

/// Override fragment metadata (`[override]` table in a fragment file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideKind {
    App { name: String },
    Urgency { level: Urgency },
}

/// One loaded behavior override fragment (may nest urgency sub-fragments).
#[derive(Debug, Clone)]
pub struct LoadedEventOverride {
    pub kind: OverrideKind,
    pub hooks: EventsHooks,
    pub nested: Vec<LoadedEventOverride>,
}

/// Runtime event policy: global hooks + override tree.
#[derive(Debug, Clone)]
pub struct EventsPolicy {
    pub base: EventsHooks,
    pub overrides: Vec<LoadedEventOverride>,
}

/// Applicable override layers for one notification, lowest → highest precedence.
#[derive(Debug, Clone, Copy)]
pub struct OverrideLayers<'a> {
    pub base_urgency: Option<&'a LoadedEventOverride>,
    pub app: Option<&'a LoadedEventOverride>,
    pub app_urgency: Option<&'a LoadedEventOverride>,
}

impl EventsHooks {
    /// Overlay `other` onto `self` — set fields in `other` replace; unset fields inherit.
    pub fn merge_from(&mut self, other: &EventsHooks) {
        merge_field(&mut self.on_action, &other.on_action);
        merge_field(&mut self.on_button_left, &other.on_button_left);
        merge_field(&mut self.on_button_middle, &other.on_button_middle);
        merge_field(&mut self.on_button_right, &other.on_button_right);
        merge_field(&mut self.on_touch, &other.on_touch);
        merge_field(&mut self.on_notify, &other.on_notify);
    }
}

impl EventsPolicy {
    pub fn resolve_layers<'a>(&'a self, app_id: &str, urgency: Urgency) -> OverrideLayers<'a> {
        let base_urgency = self
            .overrides
            .iter()
            .find(|ov| matches!(&ov.kind, OverrideKind::Urgency { level } if *level == urgency));

        let app = self
            .overrides
            .iter()
            .find(|ov| matches!(&ov.kind, OverrideKind::App { name } if name == app_id));

        let app_urgency = app.and_then(|app_ov| {
            app_ov.nested.iter().find(
                |sub| matches!(&sub.kind, OverrideKind::Urgency { level } if *level == urgency),
            )
        });

        OverrideLayers {
            base_urgency,
            app,
            app_urgency,
        }
    }

    /// Resolve hooks for a notification: base → global urgency → app → app urgency.
    pub fn resolve(&self, app_id: &str, urgency: Urgency) -> EventsHooks {
        let layers = self.resolve_layers(app_id, urgency);
        let mut out = self.base.clone();
        if let Some(ov) = layers.base_urgency {
            out.merge_from(&ov.hooks);
        }
        if let Some(ov) = layers.app {
            out.merge_from(&ov.hooks);
        }
        if let Some(ov) = layers.app_urgency {
            out.merge_from(&ov.hooks);
        }
        out
    }
}

fn merge_field(dst: &mut Option<HookArgv>, src: &Option<HookArgv>) {
    if src.is_some() {
        *dst = src.clone();
    }
}

#[cfg(test)]
mod tests;
