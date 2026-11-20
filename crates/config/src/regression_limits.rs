use codeint_model::Metric;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatioRise(f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewFindingsRule {
    AnyNewFinding,
    NumeratorRiseWithNewFindings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegressionLimit {
    pub ratio_rise: RatioRise,
    pub new_findings: NewFindingsRule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegressionLimits {
    erosion: RegressionLimit,
    duplication: RegressionLimit,
    verbosity: RegressionLimit,
    import_cycles: RegressionLimit,
}

impl RatioRise {
    pub const DEFAULT: RatioRise = RatioRise(0.01);

    pub fn ratio(self) -> f64 {
        todo!()
    }

    pub fn admits(self, _displayed_rise: f64) -> bool {
        todo!()
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
}

impl RegressionLimits {
    pub fn limit(&self, _metric: Metric) -> RegressionLimit {
        todo!()
    }
}

impl Default for RegressionLimits {
    fn default() -> RegressionLimits {
        todo!()
    }
}
