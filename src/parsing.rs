use crate::model::{
    ActiveState, LoadState, Scope, Service, SubState, display_text, valid_service_name,
};
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
            "invalid command output at line {}: {}",
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

fn output_text<'a>(output: &'a [u8], reason: &'static str) -> Result<&'a str, ParseError> {
    std::str::from_utf8(output).map_err(|error| ParseError {
        line: output[..error.valid_up_to()]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count()
            + 1,
        reason,
    })
}

pub fn parse_services(output: &[u8], scope: Scope) -> Result<Vec<Service>, ParseError> {
    let text = output_text(output, "output is not UTF-8")?;
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
        if !valid_service_name(name) {
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

/// Bound display work as well as wire bytes. Newlines remain separate journal
/// rows; terminal controls and bidi formatting inside each row are escaped.
pub fn bounded_line(text: &str) -> (String, bool) {
    let mut chars = text.chars();
    let prefix: String = chars
        .by_ref()
        .take(crate::model::DISPLAY_LINE_CHARS)
        .collect();
    let truncated = chars.next().is_some();
    let mut displayed = display_text(&prefix);
    if truncated {
        displayed.push_str(" [truncated]");
    }
    (displayed, truncated)
}

pub fn parse_properties(output: &[u8]) -> Result<crate::model::Properties, ParseError> {
    use crate::model::{Properties, Property};
    let text = output_text(output, "property output is not UTF-8")?;
    let mut properties = Properties::default();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        let error = |reason| ParseError {
            line: index + 1,
            reason,
        };
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| error("expected property=value"))?;
        let property = Property::ALL
            .into_iter()
            .find(|p| p.name() == name)
            .ok_or_else(|| error("unexpected property"))?;
        if matches!(property, Property::MainPID)
            && !value.is_empty()
            && value.parse::<u32>().is_err()
            || matches!(property, Property::ExecMainCode | Property::ExecMainStatus)
                && !value.is_empty()
                && value.parse::<i32>().is_err()
        {
            return Err(error("invalid numeric property"));
        }
        if properties
            .values
            .insert(property, bounded_line(value).0)
            .is_some()
        {
            return Err(error("duplicate property"));
        }
    }
    Ok(properties)
}

pub fn parse_journal(output: &[u8]) -> Result<crate::model::Journal, ParseError> {
    use crate::model::{JOURNAL_DISPLAY_LINES, Journal};
    let text = output_text(output, "journal output is not UTF-8")?;
    let mut journal = Journal::default();
    if text.trim().is_empty() || text.trim() == "-- No entries --" {
        return Ok(journal);
    }
    for line in text.lines() {
        if journal.lines.len() == JOURNAL_DISPLAY_LINES {
            journal.truncated = true;
            break;
        }
        let (line, truncated) = bounded_line(line);
        journal.truncated |= truncated;
        journal.lines.push(line);
    }
    Ok(journal)
}
