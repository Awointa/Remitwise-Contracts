#![no_std]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::manual_inspect)]
#![allow(dead_code)]
#![allow(unused_imports)]

//! # Cross-Contract Orchestrator
//!
//! The Cross-Contract Orchestrator coordinates automated remittance allocation across
//! multiple Soroban smart contracts in the Remitwise ecosystem. It implements atomic,
//! multi-contract operations with family wallet permission enforcement.

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, panic_with_error,
    symbol_short, Address, Env, Symbol, Vec,
};
use remitwise_common::{EventCategory, EventPriority, RemitwiseEvents};

#[cfg(test)]
mod test;

// ============================================================================
// Contract Client Interfaces for Cross-Contract Calls
// ============================================================================

#[contractclient(name = "FamilyWalletClient")]
pub trait FamilyWalletTrait {
    fn check_spending_limit(env: Env, caller: Address, amount: i128) -> bool;
}

#[contractclient(name = "RemittanceSplitClient")]
pub trait RemittanceSplitTrait {
    fn calculate_split(env: Env, total_amount: i128) -> Vec<i128>;
}

#[contractclient(name = "SavingsGoalsClient")]
pub trait SavingsGoalsTrait {
    fn add_to_goal(env: Env, caller: Address, goal_id: u32, amount: i128) -> i128;
}

#[contractclient(name = "BillPaymentsClient")]
pub trait BillPaymentsTrait {
    fn pay_bill(env: Env, caller: Address, bill_id: u32);
}

#[contractclient(name = "InsuranceClient")]
pub trait InsuranceTrait {
    fn pay_premium(env: Env, caller: Address, policy_id: u32) -> bool;
}

// ============================================================================
// Data Types
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OrchestratorError {
    PermissionDenied = 1,
    SpendingLimitExceeded = 2,
    SavingsDepositFailed = 3,
    BillPaymentFailed = 4,
    InsurancePaymentFailed = 5,
    RemittanceSplitFailed = 6,
    InvalidAmount = 7,
    InvalidContractAddress = 8,
    CrossContractCallFailed = 9,
    ReentrancyDetected = 10,
    DuplicateContractAddress = 11,
    ContractNotConfigured = 12,
    SelfReferenceNotAllowed = 13,
    NonceAlreadyUsed = 14,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ExecutionState {
    Idle = 0,
    Executing = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemittanceFlowResult {
    pub total_amount: i128,
    pub spending_amount: i128,
    pub savings_amount: i128,
    pub bills_amount: i128,
    pub insurance_amount: i128,
    pub savings_success: bool,
    pub bills_success: bool,
    pub insurance_success: bool,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemittanceFlowEvent {
    pub caller: Address,
    pub total_amount: i128,
    pub allocations: Vec<i128>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemittanceFlowErrorEvent {
    pub caller: Address,
    pub failed_step: Symbol,
    pub error_code: u32,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionStats {
    pub total_flows_executed: u64,
    pub total_flows_failed: u64,
    pub total_amount_processed: i128,
    pub last_execution: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrchestratorAuditEntry {
    pub caller: Address,
    pub operation: Symbol,
    pub amount: i128,
    pub success: bool,
    pub timestamp: u64,
    pub error_code: Option<u32>,
}

const INSTANCE_LIFETIME_THRESHOLD: u32 = 17280;
const INSTANCE_BUMP_AMOUNT: u32 = 518400;
const MAX_AUDIT_ENTRIES: u32 = 100;

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct Orchestrator;

#[contractimpl]
impl Orchestrator {
    // -----------------------------------------------------------------------
    // Reentrancy Guard
    // -----------------------------------------------------------------------

    fn acquire_execution_lock(env: &Env) -> Result<(), OrchestratorError> {
        let state: ExecutionState = env
            .storage()
            .instance()
            .get(&symbol_short!("EXEC_ST"))
            .unwrap_or(ExecutionState::Idle);

        if state == ExecutionState::Executing {
            return Err(OrchestratorError::ReentrancyDetected);
        }

        env.storage()
            .instance()
            .set(&symbol_short!("EXEC_ST"), &ExecutionState::Executing);

        Ok(())
    }

    fn release_execution_lock(env: &Env) {
        env.storage()
            .instance()
            .set(&symbol_short!("EXEC_ST"), &ExecutionState::Idle);
    }

    pub fn get_execution_state(env: Env) -> ExecutionState {
        env.storage()
            .instance()
            .get(&symbol_short!("EXEC_ST"))
            .unwrap_or(ExecutionState::Idle)
    }

    // -----------------------------------------------------------------------
    // Main Entry Points
    // -----------------------------------------------------------------------
    

    

    // ============================================================================
    // Public Functions - Individual Operations
    // ============================================================================
    // (Duplicate earlier simple-operation implementations removed; using nonce-protected
    // implementations later in the file that include replay protection and extended auditing.)

    // ============================================================================
    // Public Functions - Complete Remittance Flow
    // ============================================================================

    /// Execute a complete remittance flow with automated allocation
    ///
    /// This is the main orchestrator function that coordinates a full remittance
    /// split across all downstream contracts (savings, bills, insurance) with
    /// family wallet permission enforcement.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `caller` - Address initiating the operation (must authorize)
    /// * `total_amount` - Total remittance amount to split
    /// * `family_wallet_addr` - Address of the Family Wallet contract
    /// * `remittance_split_addr` - Address of the Remittance Split contract
    /// * `savings_addr` - Address of the Savings Goals contract
    /// * `bills_addr` - Address of the Bill Payments contract
    /// * `insurance_addr` - Address of the Insurance contract
    /// * `goal_id` - Target savings goal ID
    /// * `bill_id` - Target bill ID
    /// * `policy_id` - Target insurance policy ID
    ///
    /// # Returns
    /// Ok(RemittanceFlowResult) with execution details if successful
    /// Err(OrchestratorError) if any step fails
    ///
    /// # Gas Estimation
    /// - Base: ~5000 gas
    /// - Family wallet check: ~2000 gas
    /// - Remittance split calc: ~3000 gas
    /// - Savings deposit: ~4000 gas
    /// - Bill payment: ~4000 gas
    /// - Insurance payment: ~4000 gas
    /// - Total: ~22,000 gas for full flow
    ///
    /// # Atomicity Guarantee
    /// All operations execute atomically via Soroban's panic/revert mechanism.
    /// If any step fails, all prior state changes are automatically reverted.
    ///
    /// # Execution Flow
    /// 1. Require caller authorization
    /// 2. Validate total_amount is positive
    /// 3. Check family wallet permission
    /// 4. Check spending limit
    /// 5. Extract allocations from remittance split
    /// 6. Deposit to savings goal
    /// 7. Pay bill
    /// 8. Pay insurance premium
    /// 9. Build and return result
    /// 10. On error, emit error event and return error
    
    #[allow(clippy::too_many_arguments)]
    pub fn execute_remittance_flow(
        env: Env,
        caller: Address,
        total_amount: i128,
        family_wallet_addr: Address,
        remittance_split_addr: Address,
        savings_addr: Address,
        bills_addr: Address,
        insurance_addr: Address,
        goal_id: u32,
        bill_id: u32,
        policy_id: u32,
    ) -> Result<RemittanceFlowResult, OrchestratorError> {
        Self::acquire_execution_lock(&env)?;
        caller.require_auth();
        let timestamp = env.ledger().timestamp();

        let res = (|| {
            Self::validate_remittance_flow_addresses(
                &env,
                &family_wallet_addr,
                &remittance_split_addr,
                &savings_addr,
                &bills_addr,
                &insurance_addr,
            )?;

            if total_amount <= 0 {
                return Err(OrchestratorError::InvalidAmount);
            }

            Self::check_spending_limit(&env, &family_wallet_addr, &caller, total_amount)?;

            let allocations = Self::extract_allocations(&env, &remittance_split_addr, total_amount)?;

            let spending_amount = allocations.get(0).unwrap_or(0);
            let savings_amount = allocations.get(1).unwrap_or(0);
            let bills_amount = allocations.get(2).unwrap_or(0);
            let insurance_amount = allocations.get(3).unwrap_or(0);

            let savings_success = Self::deposit_to_savings(&env, &savings_addr, &caller, goal_id, savings_amount).is_ok();
            let bills_success = Self::execute_bill_payment_internal(&env, &bills_addr, &caller, bill_id).is_ok();
            let insurance_success = Self::pay_insurance_premium(&env, &insurance_addr, &caller, policy_id).is_ok();

            let flow_result = RemittanceFlowResult {
                total_amount,
                spending_amount,
                savings_amount,
                bills_amount,
                insurance_amount,
                savings_success,
                bills_success,
                insurance_success,
                timestamp,
            };

            Self::emit_success_event(&env, &caller, total_amount, &allocations, timestamp);
            Ok(flow_result)
        })();

        // Update stats and audit log for success/failure
        match &res {
            Ok(flow_result) => {
                Self::update_execution_stats(&env, flow_result.total_amount, true, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("flow"),
                    amount: flow_result.total_amount,
                    success: true,
                    timestamp,
                    error_code: None,
                });
            }
            Err(e) => {
                Self::update_execution_stats(&env, 0, false, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("flow"),
                    amount: 0,
                    success: false,
                    timestamp,
                    error_code: Some(*e as u32),
                });
                Self::emit_error_event(&env, &caller, symbol_short!("flow"), *e as u32, timestamp);
            }
        }

        Self::release_execution_lock(&env);
        res
    }

    pub fn execute_savings_deposit(
        env: Env,
        caller: Address,
        amount: i128,
        family_wallet_addr: Address,
        savings_addr: Address,
        goal_id: u32,
        nonce: u64,
    ) -> Result<(), OrchestratorError> {
        Self::acquire_execution_lock(&env)?;
        caller.require_auth();
        let timestamp = env.ledger().timestamp();
        // Address validation
        Self::validate_two_addresses(&env, &family_wallet_addr, &savings_addr).map_err(|e| {
            Self::release_execution_lock(&env);
            e
        })?;
        // Nonce / replay protection
        Self::consume_nonce(&env, &caller, symbol_short!("exec_sav"), nonce).map_err(|e| {
            Self::release_execution_lock(&env);
            e
        })?;

        let result = (|| {
            Self::check_spending_limit(&env, &family_wallet_addr, &caller, amount)?;
            Self::deposit_to_savings(&env, &savings_addr, &caller, goal_id, amount)?;
            Ok(())
        })();

        match &result {
            Ok(_) => {
                Self::update_execution_stats(&env, amount, true, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("exec_sav"),
                    amount,
                    success: true,
                    timestamp,
                    error_code: None,
                });
            }
            Err(e) => {
                Self::update_execution_stats(&env, amount, false, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("exec_sav"),
                    amount,
                    success: false,
                    timestamp,
                    error_code: Some(*e as u32),
                });
            }
        }

        Self::release_execution_lock(&env);
        result
    }

    pub fn execute_bill_payment(
        env: Env,
        caller: Address,
        amount: i128,
        family_wallet_addr: Address,
        bills_addr: Address,
        bill_id: u32,
        nonce: u64,
    ) -> Result<(), OrchestratorError> {
        Self::acquire_execution_lock(&env)?;
        caller.require_auth();
        let timestamp = env.ledger().timestamp();
        // Address validation
        Self::validate_two_addresses(&env, &family_wallet_addr, &bills_addr).map_err(|e| {
            Self::release_execution_lock(&env);
            e
        })?;
        // Nonce / replay protection
        Self::consume_nonce(&env, &caller, symbol_short!("exec_bill"), nonce).map_err(|e| {
            Self::release_execution_lock(&env);
            e
        })?;

        let result = (|| {
            Self::check_spending_limit(&env, &family_wallet_addr, &caller, amount)?;
            Self::execute_bill_payment_internal(&env, &bills_addr, &caller, bill_id)?;
            Ok(())
        })();
        match &result {
            Ok(_) => {
                Self::update_execution_stats(&env, amount, true, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("exec_bill"),
                    amount,
                    success: true,
                    timestamp,
                    error_code: None,
                });
            }
            Err(e) => {
                Self::update_execution_stats(&env, amount, false, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("exec_bill"),
                    amount,
                    success: false,
                    timestamp,
                    error_code: Some(*e as u32),
                });
            }
        }

        Self::release_execution_lock(&env);
        result
    }

    pub fn execute_insurance_payment(
        env: Env,
        caller: Address,
        amount: i128,
        family_wallet_addr: Address,
        insurance_addr: Address,
        policy_id: u32,
        nonce: u64,
    ) -> Result<(), OrchestratorError> {
        Self::acquire_execution_lock(&env)?;
        caller.require_auth();
        let timestamp = env.ledger().timestamp();
        // Address validation
        Self::validate_two_addresses(&env, &family_wallet_addr, &insurance_addr).map_err(|e| {
            Self::release_execution_lock(&env);
            e
        })?;
        // Nonce / replay protection
        Self::consume_nonce(&env, &caller, symbol_short!("exec_ins"), nonce).map_err(|e| {
            Self::release_execution_lock(&env);
            e
        })?;

        let result = (|| {
            Self::check_spending_limit(&env, &family_wallet_addr, &caller, amount)?;
            Self::pay_insurance_premium(&env, &insurance_addr, &caller, policy_id)?;
            Ok(())
        })();
        match &result {
            Ok(_) => {
                Self::update_execution_stats(&env, amount, true, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("exec_ins"),
                    amount,
                    success: true,
                    timestamp,
                    error_code: None,
                });
            }
            Err(e) => {
                Self::update_execution_stats(&env, amount, false, timestamp);
                Self::append_audit_entry(&env, &OrchestratorAuditEntry {
                    caller: caller.clone(),
                    operation: symbol_short!("exec_ins"),
                    amount,
                    success: false,
                    timestamp,
                    error_code: Some(*e as u32),
                });
            }
        }

        Self::release_execution_lock(&env);
        result
    }

    // -----------------------------------------------------------------------
    // Internal Helpers
    // -----------------------------------------------------------------------

    fn check_spending_limit(env: &Env, family_wallet_addr: &Address, caller: &Address, amount: i128) -> Result<(), OrchestratorError> {
        let wallet_client = FamilyWalletClient::new(env, family_wallet_addr);
        if wallet_client.check_spending_limit(caller, &amount) {
            Ok(())
        } else {
            Err(OrchestratorError::SpendingLimitExceeded)
        }
    }

    fn extract_allocations(env: &Env, split_addr: &Address, total: i128) -> Result<Vec<i128>, OrchestratorError> {
        let client = RemittanceSplitClient::new(env, split_addr);
        Ok(client.calculate_split(&total))
    }

    fn deposit_to_savings(env: &Env, addr: &Address, caller: &Address, goal_id: u32, amount: i128) -> Result<(), OrchestratorError> {
        let client = SavingsGoalsClient::new(env, addr);
        client.add_to_goal(caller, &goal_id, &amount);
        Ok(())
    }

    fn execute_bill_payment_internal(env: &Env, addr: &Address, caller: &Address, bill_id: u32) -> Result<(), OrchestratorError> {
        let client = BillPaymentsClient::new(env, addr);
        client.pay_bill(caller, &bill_id);
        Ok(())
    }

    fn pay_insurance_premium(env: &Env, addr: &Address, caller: &Address, policy_id: u32) -> Result<(), OrchestratorError> {
        let client = InsuranceClient::new(env, addr);
        client.pay_premium(caller, &policy_id);
        Ok(())
    }

    fn validate_remittance_flow_addresses(
        env: &Env,
        family: &Address,
        split: &Address,
        savings: &Address,
        bills: &Address,
        insurance: &Address,
    ) -> Result<(), OrchestratorError> {
        let current = env.current_contract_address();
        if family == &current || split == &current || savings == &current || bills == &current || insurance == &current {
            return Err(OrchestratorError::SelfReferenceNotAllowed);
        }
        if family == split || family == savings || family == bills || family == insurance ||
           split == savings || split == bills || split == insurance ||
           savings == bills || savings == insurance ||
           bills == insurance {
            return Err(OrchestratorError::DuplicateContractAddress);
        }
        Ok(())
    }

    fn emit_success_event(env: &Env, caller: &Address, total: i128, allocations: &Vec<i128>, timestamp: u64) {
        env.events().publish((symbol_short!("flow_ok"),), RemittanceFlowEvent {
            caller: caller.clone(),
            total_amount: total,
            allocations: allocations.clone(),
            timestamp,
        });
    }

    fn emit_error_event(env: &Env, caller: &Address, step: Symbol, code: u32, timestamp: u64) {
        env.events().publish((symbol_short!("flow_err"),), RemittanceFlowErrorEvent {
            caller: caller.clone(),
            failed_step: step,
            error_code: code,
            timestamp,
        });
    }

    fn update_execution_stats(env: &Env, amount: i128, success: bool, timestamp: u64) {
        let mut stats: ExecutionStats = env
            .storage()
            .instance()
            .get(&symbol_short!("STATS"))
            .unwrap_or(ExecutionStats {
                total_flows_executed: 0,
                total_flows_failed: 0,
                total_amount_processed: 0,
                last_execution: 0,
            });

        if success {
            stats.total_flows_executed = stats.total_flows_executed.saturating_add(1);
            stats.total_amount_processed = stats.total_amount_processed + amount;
        } else {
            stats.total_flows_failed = stats.total_flows_failed.saturating_add(1);
        }

        stats.last_execution = timestamp;
        env.storage().instance().set(&symbol_short!("STATS"), &stats);
    }

    fn append_audit_entry(env: &Env, entry: &OrchestratorAuditEntry) {
        let mut log: Vec<OrchestratorAuditEntry> = env
            .storage()
            .instance()
            .get(&symbol_short!("AUDIT"))
            .unwrap_or_else(|| Vec::new(&env));

        if log.len() < MAX_AUDIT_ENTRIES {
            log.push_back(entry.clone());
        } else {
            // Keep the newest (MAX_AUDIT_ENTRIES - 1) entries and append the new one
            let keep = MAX_AUDIT_ENTRIES.saturating_sub(1);
            let len = log.len();
            let start = if len > keep { len - keep } else { 0 };
            let mut new_log = Vec::new(&env);
            for i in start..len {
                if let Some(e) = log.get(i) {
                    new_log.push_back(e.clone());
                }
            }
            new_log.push_back(entry.clone());
            log = new_log;
        }

        env.storage().instance().set(&symbol_short!("AUDIT"), &log);
    }

    pub fn get_execution_stats(env: Env) -> ExecutionStats {
        env.storage().instance().get(&symbol_short!("STATS")).unwrap_or(ExecutionStats {
            total_flows_executed: 0,
            total_flows_failed: 0,
            total_amount_processed: 0,
            last_execution: 0,
        })
    }

    fn validate_two_addresses(env: &Env, a: &Address, b: &Address) -> Result<(), OrchestratorError> {
        let current = env.current_contract_address();
        if a == b { return Err(OrchestratorError::DuplicateContractAddress); }
        if *a == current || *b == current { return Err(OrchestratorError::SelfReferenceNotAllowed); }
        Ok(())
    }

    fn consume_nonce(env: &Env, caller: &Address, key: Symbol, nonce: u64) -> Result<(), OrchestratorError> {
        // Simple per-caller, per-key last-nonce store to prevent replays
        let storage_key = (caller.clone(), key);
        let last: u64 = env.storage().instance().get(&storage_key).unwrap_or(0u64);
        if nonce <= last {
            return Err(OrchestratorError::NonceAlreadyUsed);
        }
        env.storage().instance().set(&storage_key, &nonce);
        Ok(())
    }

    pub fn get_audit_log(env: Env, from_index: u32, limit: u32) -> Vec<OrchestratorAuditEntry> {
        let log: Vec<OrchestratorAuditEntry> = env.storage().instance().get(&symbol_short!("AUDIT")).unwrap_or_else(|| Vec::new(&env));
        let mut out = Vec::new(&env);
        let len = log.len();
        let end = from_index.saturating_add(limit).min(len);
        for i in from_index..end {
            if let Some(e) = log.get(i) { out.push_back(e); }
        }
        out
    }
}
