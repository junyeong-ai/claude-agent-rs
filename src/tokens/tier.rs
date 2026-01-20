use serde::{Deserialize, Serialize};

pub use crate::models::LONG_CONTEXT_THRESHOLD;

/// Default warning threshold for context window utilization (80%)
pub const DEFAULT_WARNING_THRESHOLD: f64 = 0.80;

/// Default critical threshold for context window utilization (95%)
pub const DEFAULT_CRITICAL_THRESHOLD: f64 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PricingTier {
    Standard,
    Extended,
}

impl PricingTier {
    pub fn for_context(context_tokens: u64) -> Self {
        if context_tokens <= LONG_CONTEXT_THRESHOLD {
            Self::Standard
        } else {
            Self::Extended
        }
    }

    pub fn multiplier(&self) -> f64 {
        match self {
            Self::Standard => 1.0,
            Self::Extended => 2.0,
        }
    }

    pub fn is_extended(&self) -> bool {
        matches!(self, Self::Extended)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_standard() {
        assert_eq!(PricingTier::for_context(100_000), PricingTier::Standard);
        assert_eq!(PricingTier::for_context(200_000), PricingTier::Standard);
    }

    #[test]
    fn test_tier_extended() {
        assert_eq!(PricingTier::for_context(200_001), PricingTier::Extended);
        assert_eq!(PricingTier::for_context(1_000_000), PricingTier::Extended);
    }

    #[test]
    fn test_multiplier() {
        assert_eq!(PricingTier::Standard.multiplier(), 1.0);
        assert_eq!(PricingTier::Extended.multiplier(), 2.0);
    }
}
