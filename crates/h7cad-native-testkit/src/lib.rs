#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionTier {
    Smoke,
    Core,
    Full,
}

pub const DEFAULT_REGRESSION_TIERS: [RegressionTier; 3] = [
    RegressionTier::Smoke,
    RegressionTier::Core,
    RegressionTier::Full,
];
