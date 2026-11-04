use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Scope {
    System,
    User,
}

impl Scope {
    pub fn flag(self) -> &'static str {
        match self {
            Self::System => "--system",
            Self::User => "--user",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::System => "system",
            Self::User => "user",
        })
    }
}

// Unknown future systemd states retain their original value rather than being
// misreported as a known state. Substates have meaning only with their unit type.
macro_rules! states {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant,)+
            Other(String),
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $($value => Self::$variant,)+
                    value => Self::Other(value.to_owned()),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self {
                    $(Self::$variant => $value,)+
                    Self::Other(value) => value,
                })
            }
        }
    };
}

states!(LoadState {
    Loaded => "loaded", NotFound => "not-found", Masked => "masked",
    Error => "error", BadSetting => "bad-setting", Merged => "merged", Stub => "stub",
});
states!(ActiveState {
    Active => "active", Inactive => "inactive", Failed => "failed",
    Activating => "activating", Deactivating => "deactivating", Reloading => "reloading",
    Maintenance => "maintenance", Refreshing => "refreshing",
});
states!(SubState {
    Running => "running", Exited => "exited", Dead => "dead", Failed => "failed",
    Start => "start", Stop => "stop", AutoRestart => "auto-restart",
});

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Service {
    // Identity is the pair (scope, name); both managers can have the same name.
    pub scope: Scope,
    pub name: String,
    pub description: String,
    pub load: LoadState,
    pub active: ActiveState,
    pub substate: SubState,
}

/// Escape terminal controls and bidirectional formatting in untrusted text.
/// Unit names remain escaped as supplied by systemd, never shell-decoded.
pub fn display_text(text: &str) -> String {
    let mut displayed = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control()
            || matches!(c, '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            displayed.extend(c.escape_default());
        } else {
            displayed.push(c);
        }
    }
    displayed
}
