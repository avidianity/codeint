use codeint_model::{Metric, SourceSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct RatioRise(f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NewFindingsRule {
    AnyNewFinding,
    NumeratorRiseWithNewFindings,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionLimit {
    pub ratio_rise: RatioRise,
    pub new_findings: NewFindingsRule,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RegressionLimits {
    every_source_set: Vec<(String, RegressionLimitSetting)>,
    one_source_set: Vec<(SourceSet, String, RegressionLimitSetting)>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegressionLimitSetting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest rise of the ratio, from 0 to 1, as the report shows it, rounded to 4 decimal places. A rise equal to the limit passes."
    )]
    ratio_rise: Option<RatioRise>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "NewFindingsRule",
        description = "When new Findings fail: any_new_finding fails on any new Finding; numerator_rise_with_new_findings fails only when the numerator rises too, so a Finding that only moves passes; never lets new Findings pass, so that with ratio_rise 1 this Metric never makes a Regression here."
    )]
    new_findings: Option<NewFindingsRule>,
}

impl RatioRise {
    pub const DEFAULT: RatioRise = RatioRise(0.01);

    pub fn ratio(self) -> f64 {
        self.0
    }

    pub fn admits(self, displayed_rise: f64) -> bool {
        displayed_rise <= self.0
    }
}

impl RegressionLimit {
    pub const DEFAULT: RegressionLimit = RegressionLimit {
        ratio_rise: RatioRise::DEFAULT,
        new_findings: NewFindingsRule::NumeratorRiseWithNewFindings,
    };

    pub const DEFAULT_FOR_IMPORT_CYCLES: RegressionLimit = RegressionLimit {
        ratio_rise: RatioRise::DEFAULT,
        new_findings: NewFindingsRule::AnyNewFinding,
    };

    fn default_for_key(metric_key: &str) -> RegressionLimit {
        if metric_key == Metric::ImportCycles.key() || metric_key == "boundaries" {
            RegressionLimit::DEFAULT_FOR_IMPORT_CYCLES
        } else {
            RegressionLimit::DEFAULT
        }
    }

    fn overridden_by(self, setting: Option<RegressionLimitSetting>) -> RegressionLimit {
        let Some(setting) = setting else {
            return self;
        };
        RegressionLimit {
            ratio_rise: setting.ratio_rise.unwrap_or(self.ratio_rise),
            new_findings: setting.new_findings.unwrap_or(self.new_findings),
        }
    }
}

impl RegressionLimitSetting {
    pub(crate) fn is_unset(&self) -> bool {
        self.ratio_rise.is_none() && self.new_findings.is_none()
    }
}

impl RegressionLimits {
    pub(crate) fn new(
        every_source_set: Vec<(String, RegressionLimitSetting)>,
        one_source_set: Vec<(SourceSet, String, RegressionLimitSetting)>,
    ) -> RegressionLimits {
        RegressionLimits {
            every_source_set,
            one_source_set,
        }
    }

    pub fn limit(&self, source_set: SourceSet, metric: Metric) -> RegressionLimit {
        self.limit_for_key(source_set, metric.key())
    }

    pub fn limit_for_key(&self, source_set: SourceSet, metric_key: &str) -> RegressionLimit {
        let for_every_source_set = self
            .every_source_set
            .iter()
            .find(|(set_for, _)| set_for == metric_key)
            .map(|(_, setting)| *setting);
        let for_this_source_set = self
            .one_source_set
            .iter()
            .find(|(set_in, set_for, _)| *set_in == source_set && set_for == metric_key)
            .map(|(_, _, setting)| *setting);
        RegressionLimit::default_for_key(metric_key)
            .overridden_by(for_every_source_set)
            .overridden_by(for_this_source_set)
    }

    pub(crate) fn metric_keys(&self) -> impl Iterator<Item = (Option<SourceSet>, &str)> {
        self.every_source_set
            .iter()
            .map(|(metric, _)| (None, metric.as_str()))
            .chain(
                self.one_source_set
                    .iter()
                    .map(|(source_set, metric, _)| (Some(*source_set), metric.as_str())),
            )
    }
}
