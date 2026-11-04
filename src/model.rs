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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceKey {
    pub scope: Scope,
    pub name: String,
}
impl From<&Service> for ServiceKey {
    fn from(service: &Service) -> Self {
        Self {
            scope: service.scope,
            name: service.name.clone(),
        }
    }
}

pub fn valid_service_name(name: &str) -> bool {
    name.len() <= 255
        && name != ".service"
        && name.ends_with(".service")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b":_.@\\-".contains(&b))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Property {
    Id,
    Description,
    LoadState,
    ActiveState,
    SubState,
    FragmentPath,
    UnitFileState,
    MainPID,
    ExecMainCode,
    ExecMainStatus,
    Result,
    ActiveEnterTimestamp,
}
impl Property {
    pub const ALL: [Self; 12] = [
        Self::Id,
        Self::Description,
        Self::LoadState,
        Self::ActiveState,
        Self::SubState,
        Self::FragmentPath,
        Self::UnitFileState,
        Self::MainPID,
        Self::ExecMainCode,
        Self::ExecMainStatus,
        Self::Result,
        Self::ActiveEnterTimestamp,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Id => "Id",
            Self::Description => "Description",
            Self::LoadState => "LoadState",
            Self::ActiveState => "ActiveState",
            Self::SubState => "SubState",
            Self::FragmentPath => "FragmentPath",
            Self::UnitFileState => "UnitFileState",
            Self::MainPID => "MainPID",
            Self::ExecMainCode => "ExecMainCode",
            Self::ExecMainStatus => "ExecMainStatus",
            Self::Result => "Result",
            Self::ActiveEnterTimestamp => "ActiveEnterTimestamp",
        }
    }
}

#[derive(Debug, Default)]
pub struct Properties {
    pub values: std::collections::BTreeMap<Property, String>,
    pub warning: String,
}

pub const JOURNAL_ENTRIES: usize = 100;
pub const JOURNAL_DISPLAY_LINES: usize = 2000;
pub const INSPECTION_OUTPUT_BYTES: usize = 512 * 1024;
pub const DISPLAY_LINE_CHARS: usize = 4096;

#[derive(Debug, Default)]
pub struct Journal {
    pub lines: Vec<String>,
    pub warning: String,
    pub truncated: bool,
}
