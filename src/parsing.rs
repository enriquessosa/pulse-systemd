use crate::model::{ActiveState, LoadState, Scope, Service, SubState, display_text};
use std::{collections::HashSet, fmt};

#[derive(Debug, Eq, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub reason: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid service listing at line {}: {}",
            self.line, self.reason
        )
    }
}
impl std::error::Error for ParseError {}

fn token<'a>(rest: &mut &'a str) -> Option<&'a str> {
    *rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
    if rest.is_empty() {
        return None;
    }
    let end = rest
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(rest.len());
    let value = &rest[..end];
    *rest = &rest[end..];
    Some(value)
}

pub fn parse_services(output: &[u8], scope: Scope) -> Result<Vec<Service>, ParseError> {
    let text = std::str::from_utf8(output).map_err(|error| ParseError {
        line: output[..error.valid_up_to()]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count()
            + 1,
        reason: "output is not UTF-8",
    })?;
    let mut services = Vec::new();
    let mut names = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        let error = |reason| ParseError {
            line: index + 1,
            reason,
        };
        if line.chars().any(|c| c.is_control() && c != '\t') {
            return Err(error("unexpected control character"));
        }
        if line.trim().is_empty() {
            continue;
        }
        let mut rest = line;
        let name = token(&mut rest).ok_or_else(|| error("missing unit name"))?;
        if name.len() > 255
            || name == ".service"
            || !name.ends_with(".service")
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b":_.@\\-".contains(&b))
        {
            return Err(error("invalid .service unit name"));
        }
        let mut state = || {
            let value = token(&mut rest).ok_or_else(|| error("missing state column"))?;
            if !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(error("invalid state token"));
            }
            Ok(value)
        };
        let load = LoadState::from(state()?);
        let active = ActiveState::from(state()?);
        let substate = SubState::from(state()?);
        if !names.insert(name) {
            return Err(error("duplicate unit name"));
        }
        services.push(Service {
            scope,
            name: name.to_owned(),
            description: display_text(rest.trim()),
            load,
            active,
            substate,
        });
    }
    services.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(services)
}
