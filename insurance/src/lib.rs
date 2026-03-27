#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, String, Vec,
};
use remitwise_common::{CoverageType, clamp_limit, EventCategory, EventPriority, RemitwiseEvents};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InsuranceError {
    PolicyNotFound = 1,
    Unauthorized = 2,
    InvalidPremium = 3,
    InvalidCoverage = 4,
    ScheduleNotFound = 5,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct InsurancePolicy {
    pub id: u32,
    pub owner: Address,
    pub name: String,
    pub coverage_type: CoverageType,
    pub monthly_premium: i128,
    pub coverage_amount: i128,
    pub active: bool,
    pub next_payment_date: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PolicyPage {
    pub items: Vec<InsurancePolicy>,
    pub count: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PremiumSchedule {
    pub id: u32,
    pub policy_id: u32,
    pub owner: Address,
    pub next_due: u64,
    pub interval: u64,
    pub active: bool,
    pub missed_count: u32,
}

#[contract]
pub struct Insurance;

#[contractimpl]
impl Insurance {
    pub fn create_policy(
        env: Env,
        owner: Address,
        name: String,
        coverage_type: CoverageType,
        monthly_premium: i128,
        coverage_amount: i128,
    ) -> u32 {
        owner.require_auth();
        if monthly_premium <= 0 { panic!("Monthly premium must be positive"); }
        if coverage_amount <= 0 { panic!("Coverage amount must be positive"); }
        
        let mut policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
        let id = env.storage().instance().get(&symbol_short!("NEXT_ID")).unwrap_or(0u32) + 1;
        
        let policy = InsurancePolicy {
            id,
            owner: owner.clone(),
            name,
            coverage_type,
            monthly_premium,
            coverage_amount,
            active: true,
            next_payment_date: env.ledger().timestamp() + (30 * 86400),
        };
        
        policies.set(id, policy);
        env.storage().instance().set(&symbol_short!("POLS"), &policies);
        env.storage().instance().set(&symbol_short!("NEXT_ID"), &id);

        RemitwiseEvents::emit(
            &env,
            EventCategory::State,
            EventPriority::Medium,
            symbol_short!("created"),
            (id, owner, monthly_premium),
        );

        id
    }

    pub fn get_policy(env: Env, id: u32) -> Option<InsurancePolicy> {
        let policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
        policies.get(id)
    }

    pub fn pay_premium(env: Env, owner: Address, id: u32) {
        owner.require_auth();
        let mut policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
        let mut policy = policies.get(id).expect("Policy not found");
        if policy.owner != owner { panic!("Only the policy owner can pay premiums"); }
        
        policy.next_payment_date = env.ledger().timestamp() + (30 * 86400);
        policies.set(id, policy);
        env.storage().instance().set(&symbol_short!("POLS"), &policies);
    }

    pub fn deactivate_policy(env: Env, owner: Address, id: u32) -> bool {
        owner.require_auth();
        let mut policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
        if let Some(mut policy) = policies.get(id) {
            if policy.owner != owner { return false; }
            policy.active = false;
            policies.set(id, policy);
            env.storage().instance().set(&symbol_short!("POLS"), &policies);
            return true;
        }
        false
    }

    pub fn get_active_policies(env: Env, owner: Address, offset: u32, limit: u32) -> PolicyPage {
        let limit = clamp_limit(limit);
        let policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
        let mut items = Vec::new(&env);
        let mut count = 0u32;
        let mut skipped = 0u32;
        
        for (_, p) in policies.iter() {
            if p.owner == owner && p.active {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                items.push_back(p);
                count += 1;
                if count >= limit { break; }
            }
        }
        PolicyPage { count: items.len(), items }
    }

    pub fn get_all_policies_for_owner(env: Env, owner: Address, offset: u32, limit: u32) -> PolicyPage {
        let limit = clamp_limit(limit);
        let policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
        let mut items = Vec::new(&env);
        let mut count = 0u32;
        let mut skipped = 0u32;

        for (_, p) in policies.iter() {
            if p.owner == owner {
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                items.push_back(p);
                count += 1;
                if count >= limit { break; }
            }
        }
        PolicyPage { count: items.len(), items }
    }

    pub fn get_total_monthly_premium(env: Env, owner: Address) -> i128 {
        let policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
        let mut total = 0i128;
        for (_, p) in policies.iter() {
            if p.owner == owner && p.active {
                total += p.monthly_premium;
            }
        }
        total
    }

    pub fn create_premium_schedule(env: Env, owner: Address, policy_id: u32, next_due: u64, interval: u64) -> u32 {
        owner.require_auth();
        let mut schedules: Map<u32, PremiumSchedule> = env.storage().instance().get(&symbol_short!("SCHEDS")).unwrap_or_else(|| Map::new(&env));
        let id = env.storage().instance().get(&symbol_short!("NEXT_SCH")).unwrap_or(0u32) + 1;
        
        let schedule = PremiumSchedule {
            id,
            policy_id,
            owner,
            next_due,
            interval,
            active: true,
            missed_count: 0,
        };
        
        schedules.set(id, schedule);
        env.storage().instance().set(&symbol_short!("SCHEDS"), &schedules);
        env.storage().instance().set(&symbol_short!("NEXT_SCH"), &id);
        id
    }

    pub fn get_premium_schedule(env: Env, id: u32) -> Option<PremiumSchedule> {
        let schedules: Map<u32, PremiumSchedule> = env.storage().instance().get(&symbol_short!("SCHEDS")).unwrap_or_else(|| Map::new(&env));
        schedules.get(id)
    }

    pub fn modify_premium_schedule(env: Env, owner: Address, id: u32, next_due: u64, interval: u64) {
        owner.require_auth();
        let mut schedules: Map<u32, PremiumSchedule> = env.storage().instance().get(&symbol_short!("SCHEDS")).unwrap_or_else(|| Map::new(&env));
        let mut schedule = schedules.get(id).expect("Schedule not found");
        if schedule.owner != owner { panic!("Unauthorized"); }
        schedule.next_due = next_due;
        schedule.interval = interval;
        schedules.set(id, schedule);
        env.storage().instance().set(&symbol_short!("SCHEDS"), &schedules);
    }

    pub fn cancel_premium_schedule(env: Env, owner: Address, id: u32) {
        owner.require_auth();
        let mut schedules: Map<u32, PremiumSchedule> = env.storage().instance().get(&symbol_short!("SCHEDS")).unwrap_or_else(|| Map::new(&env));
        let mut schedule = schedules.get(id).expect("Schedule not found");
        if schedule.owner != owner { panic!("Unauthorized"); }
        schedule.active = false;
        schedules.set(id, schedule);
        env.storage().instance().set(&symbol_short!("SCHEDS"), &schedules);
    }

    pub fn execute_due_premium_schedules(env: Env) -> Vec<u32> {
        let mut schedules: Map<u32, PremiumSchedule> = env.storage().instance().get(&symbol_short!("SCHEDS")).unwrap_or_else(|| Map::new(&env));
        let mut executed = Vec::new(&env);
        let now = env.ledger().timestamp();
        
        for (id, mut s) in schedules.iter() {
            if s.active && s.next_due <= now {
                let mut policies: Map<u32, InsurancePolicy> = env.storage().instance().get(&symbol_short!("POLS")).unwrap_or_else(|| Map::new(&env));
                if let Some(mut p) = policies.get(s.policy_id) {
                    p.next_payment_date = now + (30 * 86400);
                    policies.set(s.policy_id, p);
                    env.storage().instance().set(&symbol_short!("POLS"), &policies);
                }
                
                if s.interval > 0 {
                    s.next_due += s.interval;
                } else {
                    s.active = false;
                }
                schedules.set(id, s);
                executed.push_back(id);
            }
        }
        env.storage().instance().set(&symbol_short!("SCHEDS"), &schedules);
        executed
    }
}