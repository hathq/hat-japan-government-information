#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const PACKAGE_JSON: &str = include_str!("../hat.package.json");
pub const PACKAGE_ID: &str = "hat/japan-government-information";
pub const REPOSITORY_ID: &str = "hat-japan-government-information";
pub const COUNTRY_CODE: &str = "JP";
pub const OPERATION_ID: &str =
    "hathq://vocabulary/action/prepare-government-information-request/v1";
pub const INPUT_SCHEMA: &str =
    "hathq://hat-japan-government-information/government-information-request-input/v1";
pub const OUTPUT_SCHEMA: &str =
    "hathq://hat-japan-government-information/government-information-request-output/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GovernmentTopic {
    Laws,
    Tax,
    Statistics,
    Administration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OfficialSource {
    pub id: &'static str,
    pub publisher: &'static str,
    pub origin: &'static str,
    pub topic: GovernmentTopic,
    pub access: &'static str,
}

pub const OFFICIAL_SOURCES: &[OfficialSource] = &[
    OfficialSource {
        id: "e-gov-laws",
        publisher: "Digital Agency, Government of Japan",
        origin: "https://laws.e-gov.go.jp",
        topic: GovernmentTopic::Laws,
        access: "public-https",
    },
    OfficialSource {
        id: "national-tax-agency",
        publisher: "National Tax Agency, Japan",
        origin: "https://www.nta.go.jp",
        topic: GovernmentTopic::Tax,
        access: "public-https",
    },
    OfficialSource {
        id: "e-stat",
        publisher: "Statistics Bureau of Japan",
        origin: "https://www.e-stat.go.jp",
        topic: GovernmentTopic::Statistics,
        access: "public-https-or-registered-api",
    },
    OfficialSource {
        id: "e-gov-portal",
        publisher: "Digital Agency, Government of Japan",
        origin: "https://www.e-gov.go.jp",
        topic: GovernmentTopic::Administration,
        access: "public-https",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernmentInformationRequest {
    pub schema: String,
    pub jurisdiction: String,
    pub topic: GovernmentTopic,
    pub question: String,
    pub administrative_area_code: Option<String>,
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparedGovernmentInformationRequest {
    pub schema: &'static str,
    pub jurisdiction: &'static str,
    pub topic: GovernmentTopic,
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub administrative_area_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of_date: Option<String>,
    pub status: &'static str,
    pub sources: Vec<OfficialSource>,
    pub unresolved_fields: Vec<&'static str>,
    pub constraints: [&'static str; 5],
}

#[must_use]
pub fn sources_for(topic: GovernmentTopic) -> Vec<OfficialSource> {
    OFFICIAL_SOURCES
        .iter()
        .copied()
        .filter(|source| source.topic == topic)
        .collect()
}

/// Prepares a deterministic source plan while preserving every unresolved field.
///
/// # Errors
///
/// Returns a stable reason when an exact bounded input cannot be accepted. No
/// normalization, semantic completion, or jurisdiction inference is performed.
pub fn prepare_request(
    input: GovernmentInformationRequest,
) -> Result<PreparedGovernmentInformationRequest, &'static str> {
    if input.schema != INPUT_SCHEMA
        || input.jurisdiction != COUNTRY_CODE
        || input.question.is_empty()
        || input.question != input.question.trim()
        || input.question.len() > 2_000
        || input
            .administrative_area_code
            .as_deref()
            .is_some_and(|value| !is_japan_prefecture(value))
        || input
            .as_of_date
            .as_deref()
            .is_some_and(|value| !is_date(value))
    {
        return Err("government-information-request-invalid");
    }
    let mut unresolved_fields = Vec::new();
    if input.as_of_date.is_none() {
        unresolved_fields.push("as-of-date");
    }
    if input.topic == GovernmentTopic::Administration && input.administrative_area_code.is_none() {
        unresolved_fields.push("administrative-area");
    }
    Ok(PreparedGovernmentInformationRequest {
        schema: OUTPUT_SCHEMA,
        jurisdiction: COUNTRY_CODE,
        topic: input.topic,
        question: input.question,
        administrative_area_code: input.administrative_area_code,
        as_of_date: input.as_of_date,
        status: if unresolved_fields.is_empty() {
            "prepared"
        } else {
            "needs-user-input"
        },
        sources: sources_for(input.topic),
        unresolved_fields,
        constraints: [
            "official-sources-only",
            "no-legal-interpretation",
            "no-tax-determination",
            "preserve-source-and-time",
            "do-not-fill-unresolved",
        ],
    })
}

fn is_japan_prefecture(value: &str) -> bool {
    value
        .strip_prefix("JP-")
        .and_then(|code| code.parse::<u8>().ok())
        .is_some_and(|code| (1..=47).contains(&code))
        && value.len() == 5
}

fn is_date(value: &str) -> bool {
    let mut fields = value.split('-');
    let Some(year) = fields.next().and_then(|value| value.parse::<u16>().ok()) else {
        return false;
    };
    let Some(month) = fields.next().and_then(|value| value.parse::<u8>().ok()) else {
        return false;
    };
    let Some(day) = fields.next().and_then(|value| value.parse::<u8>().ok()) else {
        return false;
    };
    if fields.next().is_some() || value.len() != 10 || year == 0 {
        return false;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let maximum = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=maximum).contains(&day)
}
