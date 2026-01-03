//! Multi-tenant budget management.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;

use super::pricing::global_pricing_table;
use super::tracker::OnExceed;

#[derive(Debug)]
pub struct TenantBudget {
    pub tenant_id: String,
    max_cost_usd: f64,
    used_cost_usd: AtomicU64,
    on_exceed: OnExceed,
}

impl TenantBudget {
    fn new(tenant_id: impl Into<String>, max_cost_usd: f64) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            max_cost_usd,
            used_cost_usd: AtomicU64::new(0),
            on_exceed: OnExceed::StopBeforeNext,
        }
    }

    pub fn with_on_exceed(mut self, on_exceed: OnExceed) -> Self {
        self.on_exceed = on_exceed;
        self
    }

    pub fn record(&self, model: &str, usage: &crate::types::Usage) -> f64 {
        let cost = global_pricing_table().calculate(model, usage);
        let cost_bits = (cost * 1_000_000.0) as u64;
        self.used_cost_usd.fetch_add(cost_bits, Ordering::Relaxed);
        cost
    }

    pub fn used_cost_usd(&self) -> f64 {
        self.used_cost_usd.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    pub fn remaining(&self) -> f64 {
        (self.max_cost_usd - self.used_cost_usd()).max(0.0)
    }

    pub fn is_exceeded(&self) -> bool {
        self.used_cost_usd() >= self.max_cost_usd
    }

    pub fn should_stop(&self) -> bool {
        matches!(self.on_exceed, OnExceed::StopBeforeNext) && self.is_exceeded()
    }

    pub fn max_cost_usd(&self) -> f64 {
        self.max_cost_usd
    }

    pub fn on_exceed(&self) -> &OnExceed {
        &self.on_exceed
    }
}

#[derive(Debug, Clone, Default)]
pub struct TenantBudgetManager {
    budgets: Arc<DashMap<String, Arc<TenantBudget>>>,
}

impl TenantBudgetManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_budget(&self, tenant_id: impl Into<String>, max_cost_usd: f64) -> Arc<TenantBudget> {
        let tenant_id = tenant_id.into();
        let budget = Arc::new(TenantBudget::new(tenant_id.clone(), max_cost_usd));
        self.budgets.insert(tenant_id, Arc::clone(&budget));
        budget
    }

    pub fn set_budget_with_options(
        &self,
        tenant_id: impl Into<String>,
        max_cost_usd: f64,
        on_exceed: OnExceed,
    ) -> Arc<TenantBudget> {
        let tenant_id = tenant_id.into();
        let budget =
            Arc::new(TenantBudget::new(tenant_id.clone(), max_cost_usd).with_on_exceed(on_exceed));
        self.budgets.insert(tenant_id, Arc::clone(&budget));
        budget
    }

    pub fn get(&self, tenant_id: &str) -> Option<Arc<TenantBudget>> {
        self.budgets.get(tenant_id).map(|v| Arc::clone(&v))
    }

    pub fn record(&self, tenant_id: &str, model: &str, usage: &crate::types::Usage) -> Option<f64> {
        self.budgets
            .get(tenant_id)
            .map(|budget| budget.record(model, usage))
    }

    pub fn should_stop(&self, tenant_id: &str) -> bool {
        self.budgets
            .get(tenant_id)
            .map(|b| b.should_stop())
            .unwrap_or(false)
    }

    pub fn remove(&self, tenant_id: &str) -> Option<Arc<TenantBudget>> {
        self.budgets.remove(tenant_id).map(|(_, v)| v)
    }

    pub fn tenant_ids(&self) -> Vec<String> {
        self.budgets.iter().map(|e| e.key().clone()).collect()
    }

    pub fn summary(&self) -> Vec<TenantBudgetSummary> {
        self.budgets
            .iter()
            .map(|e| TenantBudgetSummary {
                tenant_id: e.key().clone(),
                max_cost_usd: e.value().max_cost_usd(),
                used_cost_usd: e.value().used_cost_usd(),
                remaining: e.value().remaining(),
                is_exceeded: e.value().is_exceeded(),
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct TenantBudgetSummary {
    pub tenant_id: String,
    pub max_cost_usd: f64,
    pub used_cost_usd: f64,
    pub remaining: f64,
    pub is_exceeded: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Usage;

    #[test]
    fn test_tenant_budget_manager() {
        let manager = TenantBudgetManager::new();

        manager.set_budget("tenant-a", 100.0);
        manager.set_budget("tenant-b", 50.0);

        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            ..Default::default()
        };

        // tenant-a: 1M * $3 + 0.5M * $15 = $3 + $7.5 = $10.5
        let cost = manager.record("tenant-a", "claude-sonnet-4-5", &usage);
        assert!(cost.is_some());
        assert!((cost.unwrap() - 10.5).abs() < 0.01);

        let budget_a = manager.get("tenant-a").unwrap();
        assert!((budget_a.used_cost_usd() - 10.5).abs() < 0.01);
        assert!(!budget_a.is_exceeded());

        // tenant-b unaffected
        let budget_b = manager.get("tenant-b").unwrap();
        assert!((budget_b.used_cost_usd() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_concurrent_updates() {
        use std::thread;

        let manager = TenantBudgetManager::new();
        manager.set_budget("tenant-concurrent", 10000.0);

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let m = manager.clone();
                thread::spawn(move || {
                    let usage = Usage {
                        input_tokens: 100_000,
                        output_tokens: 50_000,
                        ..Default::default()
                    };
                    for _ in 0..100 {
                        m.record("tenant-concurrent", "claude-sonnet-4-5", &usage);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let budget = manager.get("tenant-concurrent").unwrap();
        // 1000 calls * $1.05 = $1050
        assert!((budget.used_cost_usd() - 1050.0).abs() < 1.0);
    }

    #[test]
    fn test_budget_exceeded() {
        let manager = TenantBudgetManager::new();
        manager.set_budget("small-budget", 5.0);

        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            ..Default::default()
        };

        // First call: ~$10.5, exceeds $5 limit
        manager.record("small-budget", "claude-sonnet-4-5", &usage);

        assert!(manager.should_stop("small-budget"));
    }
}
