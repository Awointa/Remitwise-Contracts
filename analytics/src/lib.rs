#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Map, String, Vec,
};

// Storage TTL constants
const INSTANCE_LIFETIME_THRESHOLD: u32 = 17280; // ~1 day
const INSTANCE_BUMP_AMOUNT: u32 = 518400; // ~30 days

/// Financial breakdown by category
#[derive(Clone)]
#[contracttype]
pub struct SpendingBreakdown {
    pub spending: i128,
    pub savings: i128,
    pub bills: i128,
    pub insurance: i128,
    pub total: i128,
}

/// Monthly financial report
#[derive(Clone)]
#[contracttype]
pub struct MonthlyReport {
    pub month: u32,
    pub year: u32,
    pub total_remittances: i128,
    pub total_spending: i128,
    pub total_savings: i128,
    pub total_bills: i128,
    pub total_insurance: i128,
    pub savings_goals_progress: i128,
    pub bills_paid: u32,
    pub bills_unpaid: u32,
    pub insurance_premiums_paid: u32,
}

/// Trend analysis data
#[derive(Clone)]
#[contracttype]
pub struct TrendAnalysis {
    pub period: String, // "daily", "weekly", "monthly"
    pub spending_trend: i128, // Positive = increasing, Negative = decreasing
    pub savings_trend: i128,
    pub bills_trend: i128,
    pub insurance_trend: i128,
}

/// Financial health score components
#[derive(Clone)]
#[contracttype]
pub struct HealthScore {
    pub overall_score: u32, // 0-100
    pub savings_rate: u32, // Percentage of income saved
    pub bill_compliance: u32, // Percentage of bills paid on time
    pub insurance_coverage: u32, // Insurance payment compliance
    pub goal_progress: u32, // Average progress on savings goals
}

/// Remittance history entry
#[derive(Clone)]
#[contracttype]
pub struct RemittanceHistory {
    pub timestamp: u64,
    pub amount: i128,
    pub split: SpendingBreakdown,
}

#[contract]
pub struct Analytics;

#[contractimpl]
impl Analytics {
    /// Calculate monthly spending vs saving breakdown
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    /// * `month` - Month (1-12)
    /// * `year` - Year
    ///
    /// # Returns
    /// SpendingBreakdown with category-wise amounts
    pub fn get_monthly_breakdown(
        env: Env,
        _owner: Address,
        _month: u32,
        _year: u32,
    ) -> SpendingBreakdown {
        // This would typically query other contracts
        // For now, return a placeholder structure
        // In production, this would make cross-contract calls
        
        Self::extend_instance_ttl(&env);
        
        SpendingBreakdown {
            spending: 0,
            savings: 0,
            bills: 0,
            insurance: 0,
            total: 0,
        }
    }

    /// Get remittance history for a user
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    /// * `start_timestamp` - Start of time range
    /// * `end_timestamp` - End of time range
    ///
    /// # Returns
    /// Vec of RemittanceHistory entries
    pub fn get_remittance_history(
        env: Env,
        owner: Address,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Vec<RemittanceHistory> {
        Self::extend_instance_ttl(&env);
        
        let history_key = symbol_short!("HISTORY");
        let history_map: Map<Address, Vec<RemittanceHistory>> = env
            .storage()
            .instance()
            .get(&history_key)
            .unwrap_or_else(|| Map::new(&env));

        let user_history = history_map.get(owner.clone()).unwrap_or_else(|| Vec::new(&env));
        
        let mut filtered = Vec::new(&env);
        for entry in user_history.iter() {
            if entry.timestamp >= start_timestamp && entry.timestamp <= end_timestamp {
                filtered.push_back(entry);
            }
        }
        
        filtered
    }

    /// Track a remittance for analytics
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    /// * `amount` - Total remittance amount
    /// * `split` - Spending breakdown
    pub fn track_remittance(
        env: Env,
        owner: Address,
        amount: i128,
        split: SpendingBreakdown,
    ) -> bool {
        Self::extend_instance_ttl(&env);
        
        let history_key = symbol_short!("HISTORY");
        let mut history_map: Map<Address, Vec<RemittanceHistory>> = env
            .storage()
            .instance()
            .get(&history_key)
            .unwrap_or_else(|| Map::new(&env));

        let mut user_history = history_map.get(owner.clone()).unwrap_or_else(|| Vec::new(&env));
        
        let entry = RemittanceHistory {
            timestamp: env.ledger().timestamp(),
            amount,
            split: split.clone(),
        };
        
        user_history.push_back(entry);
        history_map.set(owner, user_history);
        env.storage().instance().set(&history_key, &history_map);
        
        true
    }

    /// Get savings goal progress tracking
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    ///
    /// # Returns
    /// Total progress across all goals (current_amount / target_amount * 100)
    pub fn get_savings_goal_progress(env: Env, _owner: Address) -> u32 {
        // This would query the savings_goals contract
        // For now, return placeholder
        Self::extend_instance_ttl(&env);
        0
    }

    /// Calculate bill payment compliance rate
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    ///
    /// # Returns
    /// Compliance rate as percentage (0-100)
    pub fn get_bill_compliance_rate(env: Env, _owner: Address) -> u32 {
        // This would query the bill_payments contract
        // For now, return placeholder
        Self::extend_instance_ttl(&env);
        0
    }

    /// Get insurance premium payment history
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    ///
    /// # Returns
    /// Number of premiums paid on time
    pub fn get_insurance_payment_history(env: Env, _owner: Address) -> u32 {
        // This would query the insurance contract
        // For now, return placeholder
        Self::extend_instance_ttl(&env);
        0
    }

    /// Calculate financial health score
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    ///
    /// # Returns
    /// HealthScore with overall score and components
    pub fn calculate_health_score(env: Env, owner: Address) -> HealthScore {
        Self::extend_instance_ttl(&env);
        
        // Calculate components (would query other contracts in production)
        let savings_rate = Self::get_savings_goal_progress(env.clone(), owner.clone());
        let bill_compliance = Self::get_bill_compliance_rate(env.clone(), owner.clone());
        let insurance_coverage = Self::get_insurance_payment_history(env.clone(), owner.clone());
        let goal_progress = Self::get_savings_goal_progress(env.clone(), owner.clone());
        
        // Calculate overall score (weighted average)
        let overall_score = (savings_rate + bill_compliance + insurance_coverage + goal_progress) / 4;
        
        HealthScore {
            overall_score,
            savings_rate,
            bill_compliance,
            insurance_coverage,
            goal_progress,
        }
    }

    /// Get trend analysis for a period
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    /// * `period` - "daily", "weekly", or "monthly"
    /// * `periods_back` - Number of periods to analyze
    ///
    /// # Returns
    /// TrendAnalysis showing trends
    pub fn get_trend_analysis(
        env: Env,
        _owner: Address,
        period: String,
        _periods_back: u32,
    ) -> TrendAnalysis {
        Self::extend_instance_ttl(&env);
        
        // This would analyze historical data
        // For now, return placeholder
        TrendAnalysis {
            period: period.clone(),
            spending_trend: 0,
            savings_trend: 0,
            bills_trend: 0,
            insurance_trend: 0,
        }
    }

    /// Generate monthly report
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    /// * `month` - Month (1-12)
    /// * `year` - Year
    ///
    /// # Returns
    /// MonthlyReport with all financial data
    pub fn generate_monthly_report(
        env: Env,
        owner: Address,
        month: u32,
        year: u32,
    ) -> MonthlyReport {
        Self::extend_instance_ttl(&env);
        
        let breakdown = Self::get_monthly_breakdown(env.clone(), owner.clone(), month, year);
        
        MonthlyReport {
            month,
            year,
            total_remittances: breakdown.total,
            total_spending: breakdown.spending,
            total_savings: breakdown.savings,
            total_bills: breakdown.bills,
            total_insurance: breakdown.insurance,
            savings_goals_progress: Self::get_savings_goal_progress(env.clone(), owner.clone()) as i128,
            bills_paid: 0, // Would query bill_payments contract
            bills_unpaid: 0, // Would query bill_payments contract
            insurance_premiums_paid: Self::get_insurance_payment_history(env, owner),
        }
    }

    /// Get category-wise spending analysis
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    /// * `start_timestamp` - Start of time range
    /// * `end_timestamp` - End of time range
    ///
    /// # Returns
    /// SpendingBreakdown aggregated over the period
    pub fn get_category_analysis(
        env: Env,
        owner: Address,
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> SpendingBreakdown {
        Self::extend_instance_ttl(&env);
        
        let history = Self::get_remittance_history(env, owner, start_timestamp, end_timestamp);
        
        let mut total = SpendingBreakdown {
            spending: 0,
            savings: 0,
            bills: 0,
            insurance: 0,
            total: 0,
        };
        
        for entry in history.iter() {
            total.spending += entry.split.spending;
            total.savings += entry.split.savings;
            total.bills += entry.split.bills;
            total.insurance += entry.split.insurance;
            total.total += entry.amount;
        }
        
        total
    }

    /// Get comparative analysis (month-over-month)
    ///
    /// # Arguments
    /// * `owner` - Address of the user
    /// * `current_month` - Current month (1-12)
    /// * `current_year` - Current year
    ///
    /// # Returns
    /// Tuple of (current_month_report, previous_month_report)
    pub fn get_comparative_analysis(
        env: Env,
        owner: Address,
        current_month: u32,
        current_year: u32,
    ) -> (MonthlyReport, Option<MonthlyReport>) {
        Self::extend_instance_ttl(&env);
        
        let current = Self::generate_monthly_report(env.clone(), owner.clone(), current_month, current_year);
        
        let (prev_month, prev_year) = if current_month == 1 {
            (12, current_year - 1)
        } else {
            (current_month - 1, current_year)
        };
        
        let previous = Self::generate_monthly_report(env, owner, prev_month, prev_year);
        
        (current, Some(previous))
    }

    /// Extend the TTL of instance storage
    fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_track_remittance() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Analytics);
        let client = AnalyticsClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let split = SpendingBreakdown {
            spending: 500,
            savings: 300,
            bills: 150,
            insurance: 50,
            total: 1000,
        };

        let result = client.track_remittance(&owner, &1000i128, &split);
        assert!(result);
    }

    #[test]
    fn test_calculate_health_score() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Analytics);
        let client = AnalyticsClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let score = client.calculate_health_score(&owner);

        assert!(score.overall_score <= 100);
    }

    #[test]
    fn test_get_category_analysis() {
        let env = Env::default();
        let contract_id = env.register_contract(None, Analytics);
        let client = AnalyticsClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let current_time = env.ledger().timestamp();
        // Use a safe calculation to avoid underflow
        let days_ago: u64 = 30;
        let start = if current_time > days_ago * 86400 {
            current_time - (days_ago * 86400)
        } else {
            0
        };
        let end = current_time;

        let analysis = client.get_category_analysis(&owner, &start, &end);
        assert!(analysis.total >= 0);
    }
}

