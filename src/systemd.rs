use crate::{
    model::{Scope, Service, display_text},
    parsing::{ParseError, parse_services},
    process::{RunError, Runner},
};
use std::{fmt, sync::atomic::AtomicBool};

#[derive(Debug)]
pub enum DiscoveryError {
    Process(RunError),
    CommandFailed {
        code: Option<i32>,
        diagnostic: String,
    },
    Parse(ParseError),
}
impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(error) => error.fmt(f),
            Self::CommandFailed { code, diagnostic } => {
                write!(f, "systemctl failed (exit {code:?}): {diagnostic}")
            }
            Self::Parse(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Process(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::CommandFailed { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct Discovery {
    pub services: Vec<Service>,
    /// Successful commands may still report warnings; do not hide them.
    pub warning: String,
}

pub struct ScopeDiscovery {
    pub scope: Scope,
    pub result: Result<Discovery, DiscoveryError>,
}

pub struct Systemd<R> {
    runner: R,
}

impl<R: Runner> Systemd<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    pub fn discover(
        &self,
        scope: Scope,
        cancelled: &AtomicBool,
    ) -> Result<Discovery, DiscoveryError> {
        let output = self
            .runner
            .run(
                "systemctl",
                &[
                    scope.flag(),
                    "--no-pager",
                    "--no-legend",
                    "--plain",
                    "--full",
                    "--all",
                    "--type=service",
                    "list-units",
                ],
                cancelled,
            )
            .map_err(DiscoveryError::Process)?;
        if !output.success {
            // Report observed diagnostics without guessing whether a missing bus
            // means a stopped manager, denied access, or an unsupported session.
            let diagnostic = if output.stderr.is_empty() {
                &output.stdout
            } else {
                &output.stderr
            };
            return Err(DiscoveryError::CommandFailed {
                code: output.code,
                diagnostic: display_text(&String::from_utf8_lossy(diagnostic)),
            });
        }
        Ok(Discovery {
            services: parse_services(&output.stdout, scope).map_err(DiscoveryError::Parse)?,
            warning: display_text(&String::from_utf8_lossy(&output.stderr)),
        })
    }

    /// Preserve each manager's result independently so one failed scope cannot
    /// make the other appear empty. Call from a worker, never the UI event loop.
    pub fn discover_all(&self, cancelled: &AtomicBool) -> [ScopeDiscovery; 2] {
        [Scope::System, Scope::User].map(|scope| ScopeDiscovery {
            scope,
            result: self.discover(scope, cancelled),
        })
    }
}
