use std::collections::HashSet;

use codeint_config::{
    AgentFormat, BaselineFormat, ConfigFormat, DoctorFormat, FindingsShown, GuardFormat,
    ImpactFormat, RatchetFormat, TraceFormat,
};
use codeint_model::{ReferenceRange, ReferenceRanges, SourceSet};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::baseline::{BaselineScope, BaselineWeight, Baselined, BudgetedRatio, Identified};
use crate::ownership::{OwnedFinding, OwnershipOptions};

const DECIMAL_SCALE: f64 = 10_000.0;
const NOT_PUBLISHED_FOR_THIS_LANGUAGE: &str = "no published Reference range for this language";
const NOT_SHOWN_FOR_SEVERAL_LANGUAGES: &str =
    "not shown, because the measured files are in more than one language";
pub(crate) const NOT_SHOWN_FOR_NEAR_CLONES: &str = "not shown, because near-clone detection is enabled (Reference ranges are calibrated for exact Clone groups)";
pub(crate) const NO_IMPORTS_METRIC_NOTE: &str = "This language has no imports, so Import cycles, Orphan files, Import boundaries, and Dependency hygiene are not measured.";
pub(crate) const TEST_GAP_NO_REPORT_NOTE: &str =
    "Test gap is not measured because no Coverage report was named.";
pub(crate) const TEST_GAP_NO_LANGUAGE_COVERAGE_NOTE: &str =
    "Test gap is not measured because no named Coverage report covers this language.";
pub(crate) const TEST_GAP_NO_REPO_COVERAGE_NOTE: &str =
    "Test gap is not measured because no named Coverage report covers any measured language.";
pub(crate) const TEST_GAP_DIFF_NOTE: &str = "Test gap is left out of codeint diff because a Coverage report exists only for the code under measurement.";
pub(crate) const TEST_GAP_TREND_NOTE: &str = "Test gap is left out of codeint trend because a Coverage report exists only for the code under measurement.";

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("cannot render the report as TOON")]
    Toon(#[source] toon_format::ToonError),
    #[error("cannot render the report as JSON")]
    Json(#[source] serde_json::Error),
    #[error("cannot render the report as SARIF")]
    Sarif(#[source] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Decimal(pub(crate) f64);

impl Decimal {
    pub(crate) fn rounded(value: f64) -> Decimal {
        let rounded = (value * DECIMAL_SCALE).round() / DECIMAL_SCALE;
        let without_negative_zero = if rounded == 0.0 { 0.0 } else { rounded };
        Decimal(without_negative_zero)
    }
}

pub(crate) enum ReferenceRangeScope {
    OneLanguage(ReferenceRanges),
    SeveralLanguages,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ReferenceRangeEntry {
    Published {
        low: Decimal,
        high: Decimal,
        source: String,
    },
    NotShown(String),
}

impl ReferenceRangeScope {
    pub(crate) fn entry(&self, metric: &str, metric_version: u32) -> ReferenceRangeEntry {
        match self {
            ReferenceRangeScope::OneLanguage(ranges) => match ranges.for_metric(metric) {
                None => ReferenceRangeEntry::NotShown(NOT_PUBLISHED_FOR_THIS_LANGUAGE.to_owned()),
                Some(range) if range.is_stale_for(metric_version) => ReferenceRangeEntry::NotShown(
                    format_stale_reference_range_note(range.metric_version, metric_version),
                ),
                Some(range) => ReferenceRangeEntry::from(range),
            },
            ReferenceRangeScope::SeveralLanguages => {
                ReferenceRangeEntry::NotShown(NOT_SHOWN_FOR_SEVERAL_LANGUAGES.to_owned())
            }
        }
    }
}

fn format_stale_reference_range_note(found_version: u32, current_version: u32) -> String {
    format!(
        "not shown, because the Reference range targets Metric version {found_version}, which does not match Metric version {current_version}"
    )
}

impl From<&ReferenceRange> for ReferenceRangeEntry {
    fn from(range: &ReferenceRange) -> ReferenceRangeEntry {
        ReferenceRangeEntry::Published {
            low: Decimal::rounded(range.low),
            high: Decimal::rounded(range.high),
            source: range.source.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(crate) struct PerSourceSet<T> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    production: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    generated: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vendored: Option<T>,
}

impl<T> Default for PerSourceSet<T> {
    fn default() -> PerSourceSet<T> {
        PerSourceSet {
            production: None,
            test: None,
            generated: None,
            vendored: None,
        }
    }
}

impl<T> PerSourceSet<T> {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (SourceSet, &T)> {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
        .into_iter()
        .filter_map(|(source_set, section)| Some((source_set, section.as_ref()?)))
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        [
            &mut self.production,
            &mut self.test,
            &mut self.generated,
            &mut self.vendored,
        ]
        .into_iter()
        .flatten()
    }

    pub(crate) fn named(&self) -> Vec<(&'static str, &T)> {
        self.iter()
            .map(|(source_set, section)| (source_set_label(source_set), section))
            .collect()
    }

    pub(crate) fn production(&self) -> Option<&T> {
        self.production.as_ref()
    }

    pub(crate) fn slot(&mut self, source_set: SourceSet) -> &mut Option<T> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }
}

fn source_set_label(source_set: SourceSet) -> &'static str {
    match source_set {
        SourceSet::Production => "Production",
        SourceSet::Test => "Test",
        SourceSet::Generated => "Generated",
        SourceSet::Vendored => "Vendored",
    }
}

pub(crate) trait FindingKind: Serialize {
    const LIST_KEY: &'static str;
    const LEFT_OUT_KEY: &'static str = "findings_left_out";
    const BASELINED_KEY: &'static str = "baselined_findings";
    const SKIP_WHEN_EMPTY: bool = false;
}

#[derive(Debug)]
pub(crate) struct Findings<T> {
    pub(crate) listed: Vec<T>,
    pub(crate) left_out: usize,
    pub(crate) baselined: Baselined,
}

impl<T> Findings<T> {
    pub(crate) fn worst_first(
        worst_first: impl ExactSizeIterator<Item = T>,
        shown: FindingsShown,
    ) -> Findings<T> {
        let total = worst_first.len();
        let listed: Vec<T> = match shown {
            FindingsShown::WorstPerMetric(limit) => worst_first.take(limit).collect(),
            FindingsShown::All => worst_first.collect(),
        };
        Findings {
            left_out: total - listed.len(),
            listed,
            baselined: Baselined::default(),
        }
    }

    pub(crate) fn listing(listed: Vec<T>, left_out: usize) -> Findings<T> {
        Findings {
            listed,
            left_out,
            baselined: Baselined::default(),
        }
    }

    pub(crate) fn budgeted(&self, ratio: Decimal) -> BudgetedRatio {
        match self.baselined.retained_share {
            Some(share) => BudgetedRatio {
                ratio: Decimal::rounded(ratio.0 * share).0,
                non_baselined: true,
            },
            None => BudgetedRatio::from(ratio.0),
        }
    }

    pub(crate) fn worst_first_owned(
        items: impl Iterator<Item = T>,
        ownership: OwnershipOptions<'_>,
        shown: FindingsShown,
    ) -> Findings<T>
    where
        T: OwnedFinding,
    {
        let filtered = ownership.apply(items.collect());
        Findings::worst_first(filtered.into_iter(), shown)
    }
}

impl<T: FindingKind> Serialize for Findings<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if !(T::SKIP_WHEN_EMPTY && self.listed.is_empty() && self.left_out == 0) {
            map.serialize_entry(T::LIST_KEY, &self.listed)?;
        }
        if self.left_out > 0 {
            map.serialize_entry(T::LEFT_OUT_KEY, &self.left_out)?;
        }
        if self.baselined.hidden > 0 {
            map.serialize_entry(T::BASELINED_KEY, &self.baselined.hidden)?;
        }
        map.end()
    }
}

impl<'de, T: FindingKind + Deserialize<'de>> Findings<T> {
    fn read_entry<A: serde::de::MapAccess<'de>>(
        &mut self,
        key: &str,
        map: &mut A,
    ) -> Result<bool, A::Error> {
        if key == T::LIST_KEY {
            self.listed = map.next_value()?;
            return Ok(true);
        }
        if key == T::LEFT_OUT_KEY {
            self.left_out = map.next_value()?;
        } else if key == T::BASELINED_KEY {
            self.baselined.hidden = map.next_value()?;
        } else {
            map.next_value::<serde::de::IgnoredAny>()?;
        }
        Ok(false)
    }
}

impl<'de, T: FindingKind + Deserialize<'de>> Deserialize<'de> for Findings<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FindingsVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T: FindingKind + Deserialize<'de>> serde::de::Visitor<'de> for FindingsVisitor<T> {
            type Value = Findings<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "findings map with {}", T::LIST_KEY)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut findings = Findings::listing(Vec::new(), 0);
                let mut listed_seen = false;
                while let Some(key) = map.next_key::<String>()? {
                    listed_seen |= findings.read_entry(&key, &mut map)?;
                }
                if listed_seen || T::SKIP_WHEN_EMPTY {
                    Ok(findings)
                } else {
                    Err(serde::de::Error::missing_field(T::LIST_KEY))
                }
            }
        }

        deserializer.deserialize_map(FindingsVisitor(std::marker::PhantomData))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FindingsView<'a> {
    pub(crate) shown: FindingsShown,
    pub(crate) ownership: OwnershipOptions<'a>,
    pub(crate) baseline: Option<BaselineScope<'a>>,
}

impl FindingsView<'_> {
    pub(crate) fn map_baselined<'a, T: 'a, R>(self, metric: &str, items: &'a [T]) -> Findings<R>
    where
        R: From<&'a T> + OwnedFinding + Identified + BaselineWeight,
    {
        self.map_baselined_with_hidden(metric, items).0
    }

    pub(crate) fn map_baselined_with_hidden<'a, T: 'a, R>(
        self,
        metric: &str,
        items: &'a [T],
    ) -> (Findings<R>, HashSet<String>)
    where
        R: From<&'a T> + OwnedFinding + Identified + BaselineWeight,
    {
        let rows: Vec<R> = items.iter().map(R::from).collect();
        let (visible, baselined, hidden_ids) = match self.baseline {
            Some(scope) => scope.split(metric, rows),
            None => (rows, Baselined::default(), HashSet::new()),
        };
        (
            Findings {
                baselined,
                ..self.select(visible.into_iter())
            },
            hidden_ids,
        )
    }

    pub(crate) fn select<T: OwnedFinding>(self, items: impl Iterator<Item = T>) -> Findings<T> {
        Findings::worst_first_owned(items, self.ownership, self.shown)
    }

    pub(crate) fn map<'a, T: 'a, R: From<&'a T> + OwnedFinding>(
        self,
        items: &'a [T],
    ) -> Findings<R> {
        self.select(items.iter().map(R::from))
    }
}

pub(crate) fn format_unmatched_coverage_note(paths: &[String]) -> Option<String> {
    match paths {
        [] => None,
        [only] => Some(format!(
            "Coverage report path matched no repository file unambiguously: {only}."
        )),
        many => Some(format!(
            "Coverage report paths matched no repository file unambiguously: {}.",
            many.join(", ")
        )),
    }
}

pub(crate) enum TextReportFormat {
    Toon,
    Json,
    Text,
}

macro_rules! impl_text_report_format {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for TextReportFormat {
                fn from(format: $ty) -> TextReportFormat {
                    match format {
                        <$ty>::Toon => TextReportFormat::Toon,
                        <$ty>::Json => TextReportFormat::Json,
                        <$ty>::Text => TextReportFormat::Text,
                    }
                }
            }
        )+
    };
}

impl_text_report_format!(
    AgentFormat,
    BaselineFormat,
    ConfigFormat,
    DoctorFormat,
    GuardFormat,
    ImpactFormat,
    RatchetFormat,
    TraceFormat,
);

pub(crate) fn render_text_report<T: Serialize>(
    value: &T,
    format: TextReportFormat,
    render_text: impl FnOnce(&T) -> String,
) -> Result<String, ReportError> {
    match format {
        TextReportFormat::Toon => render_toon(value),
        TextReportFormat::Json => render_json(value),
        TextReportFormat::Text => Ok(render_text(value)),
    }
}

pub(crate) fn render_toon<T: Serialize>(value: &T) -> Result<String, ReportError> {
    toon_format::encode_default(value).map_err(ReportError::Toon)
}

pub(crate) fn render_json<T: Serialize>(value: &T) -> Result<String, ReportError> {
    serde_json::to_string_pretty(value).map_err(ReportError::Json)
}
