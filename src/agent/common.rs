//! Common agent execution utilities shared between execution and streaming.

use tracing::warn;

use crate::budget::{BudgetTracker, TenantBudget};

use super::config::BudgetConfig;

pub struct BudgetContext<'a> {
    pub tracker: &'a BudgetTracker,
    pub tenant: Option<&'a TenantBudget>,
    pub config: &'a BudgetConfig,
}

impl BudgetContext<'_> {
    pub fn check(&self) -> Result<(), crate::Error> {
        if self.tracker.should_stop() {
            let status = self.tracker.check();
            warn!(used = ?status.used(), "Budget exceeded, stopping execution");
            return Err(crate::Error::BudgetExceeded {
                used: status.used(),
                limit: self.config.max_cost_usd.unwrap_or(0.0),
            });
        }

        if let Some(tenant_budget) = self.tenant
            && tenant_budget.should_stop()
        {
            warn!(
                tenant_id = %tenant_budget.tenant_id,
                used = tenant_budget.used_cost_usd(),
                "Tenant budget exceeded, stopping execution"
            );
            return Err(crate::Error::BudgetExceeded {
                used: tenant_budget.used_cost_usd(),
                limit: tenant_budget.max_cost_usd(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_context_check_ok() {
        let tracker = BudgetTracker::new(10.0);
        let config = BudgetConfig::default();
        let ctx = BudgetContext {
            tracker: &tracker,
            tenant: None,
            config: &config,
        };
        assert!(ctx.check().is_ok());
    }
}
