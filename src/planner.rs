use crate::{DocumentSource, Error, Result};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlannerKind {
    Day,
    Notes,
}

impl PlannerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Notes => "notes",
        }
    }
}

pub type PlannerIndex = BTreeMap<String, BTreeMap<PlannerKind, usize>>;

/// Index dated planner headers. `Notes` labels the day view, `Day` the notes view.
/// The last matching page wins. Unreadable pages and impossible dates are skipped.
pub fn index_planner_dates<'a>(
    source: impl Into<DocumentSource<'a>>,
    year: i32,
) -> Result<PlannerIndex> {
    if year <= 0 {
        return Err(Error::InvalidYear);
    }
    let doc = source.into().resolve()?;
    let mut index = PlannerIndex::new();
    for page in 0..doc.num_pages() {
        let Ok(text) = doc.text(page as isize) else {
            continue;
        };
        let Some((date, kind)) = parse_header(&text, year) else {
            continue;
        };
        index.entry(date).or_default().insert(kind, page);
    }
    Ok(index)
}

fn parse_header(text: &str, year: i32) -> Option<(String, PlannerKind)> {
    let tokens: Vec<_> = text.split_whitespace().collect();
    if tokens.len() < 3 {
        return None;
    }
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == tokens[0])?
        + 1;
    let day: u8 = tokens[1].parse().ok()?;
    let days = match month {
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day < 1 || day > days {
        return None;
    }
    if !tokens.iter().any(|t| {
        [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ]
        .contains(t)
    }) {
        return None;
    }
    let kind = if tokens.contains(&"Notes") {
        PlannerKind::Day
    } else if tokens.contains(&"Day") {
        PlannerKind::Notes
    } else {
        return None;
    };
    Some((format!("{year:04}-{month:02}-{day:02}"), kind))
}
