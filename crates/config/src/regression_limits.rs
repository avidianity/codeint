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
}

impl RegressionLimits {
    pub fn limit(&self, metric: Metric) -> RegressionLimit {
        match metric {
            Metric::Erosion => self.erosion,
            Metric::Duplication => self.duplication,
            Metric::Verbosity => self.verbosity,
            Metric::ImportCycles => self.import_cycles,
        }
    }
}

impl Default for RegressionLimits {
    fn default() -> RegressionLimits {
        RegressionLimits {
            erosion: RegressionLimit::DEFAULT,
            duplication: RegressionLimit::DEFAULT,
            verbosity: RegressionLimit::DEFAULT,
            import_cycles: RegressionLimit::DEFAULT_FOR_IMPORT_CYCLES,
        }
    }
}
