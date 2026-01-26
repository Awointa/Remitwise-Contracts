#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Map, Vec,
};

// Storage TTL constants
const INSTANCE_LIFETIME_THRESHOLD: u32 = 17280; // ~1 day
const INSTANCE_BUMP_AMOUNT: u32 = 518400; // ~30 days

/// Schedule frequency types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum ScheduleFrequency {
    Weekly = 7,
    BiWeekly = 14,
    Monthly = 30,
    Custom = 0, // Custom uses frequency_days
}

/// Recurring remittance schedule configuration
#[derive(Clone)]
#[contracttype]
pub struct RemittanceSchedule {
    pub id: u32,
    pub owner: Address,
    pub amount: i128,
    pub split_config_id: Option<u32>, // Reference to split configuration
    pub frequency: ScheduleFrequency,
    pub frequency_days: u32, // For custom frequency
    pub start_timestamp: u64,
    pub end_timestamp: Option<u64>, // None means no end date
    pub active: bool,
    pub last_executed: Option<u64>,
    pub next_execution: u64,
    pub created_at: u64,
}

/// Events emitted by the contract
#[contracttype]
#[derive(Clone)]
pub enum ScheduleEvent {
    Created,
    Executed,
    Paused,
    Resumed,
    Modified,
    Cancelled,
}

#[contract]
pub struct RecurringRemittance;

#[contractimpl]
impl RecurringRemittance {
    /// Create a new recurring remittance schedule
    ///
    /// # Arguments
    /// * `owner` - Address of the schedule owner (must authorize)
    /// * `amount` - Remittance amount per execution (must be positive)
    /// * `split_config_id` - Optional split configuration ID
    /// * `frequency` - Schedule frequency (Weekly, BiWeekly, Monthly, Custom)
    /// * `frequency_days` - Days between remittances (required if frequency is Custom)
    /// * `start_timestamp` - When to start the schedule
    /// * `end_timestamp` - Optional end date (None for indefinite)
    ///
    /// # Returns
    /// The ID of the created schedule
    pub fn create_schedule(
        env: Env,
        owner: Address,
        amount: i128,
        split_config_id: Option<u32>,
        frequency: ScheduleFrequency,
        frequency_days: u32,
        start_timestamp: u64,
        end_timestamp: Option<u64>,
    ) -> u32 {
        owner.require_auth();

        if amount <= 0 {
            panic!("Amount must be positive");
        }

        if frequency == ScheduleFrequency::Custom && frequency_days == 0 {
            panic!("Custom frequency requires frequency_days > 0");
        }

        let current_time = env.ledger().timestamp();
        if start_timestamp < current_time {
            panic!("Start timestamp must be in the future");
        }

        if let Some(end) = end_timestamp {
            if end <= start_timestamp {
                panic!("End timestamp must be after start timestamp");
            }
        }

        Self::extend_instance_ttl(&env);

        let mut schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let next_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32)
            + 1;

        let schedule = RemittanceSchedule {
            id: next_id,
            owner: owner.clone(),
            amount,
            split_config_id,
            frequency,
            frequency_days,
            start_timestamp,
            end_timestamp,
            active: true,
            last_executed: None,
            next_execution: start_timestamp,
            created_at: current_time,
        };

        schedules.set(next_id, schedule);
        env.storage()
            .instance()
            .set(&symbol_short!("SCHEDULES"), &schedules);
        env.storage()
            .instance()
            .set(&symbol_short!("NEXT_ID"), &next_id);

        env.events().publish(
            (symbol_short!("schedule"), ScheduleEvent::Created),
            (next_id, owner),
        );

        next_id
    }

    /// Execute a scheduled remittance (called by external trigger)
    ///
    /// # Arguments
    /// * `schedule_id` - ID of the schedule to execute
    ///
    /// # Returns
    /// True if execution was successful
    pub fn execute_schedule(env: Env, schedule_id: u32) -> bool {
        Self::extend_instance_ttl(&env);

        let mut schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let mut schedule = schedules.get(schedule_id).expect("Schedule not found");

        if !schedule.active {
            panic!("Schedule is not active");
        }

        let current_time = env.ledger().timestamp();
        if current_time < schedule.next_execution {
            panic!("Schedule not ready for execution");
        }

        if let Some(end) = schedule.end_timestamp {
            if current_time > end {
                schedule.active = false;
                schedules.set(schedule_id, schedule);
                env.storage()
                    .instance()
                    .set(&symbol_short!("SCHEDULES"), &schedules);
                return false;
            }
        }

        schedule.last_executed = Some(current_time);

        let days = match schedule.frequency {
            ScheduleFrequency::Custom => schedule.frequency_days,
            _ => schedule.frequency as u32,
        };

        schedule.next_execution = current_time + (days as u64 * 86400);

        schedules.set(schedule_id, schedule);
        env.storage()
            .instance()
            .set(&symbol_short!("SCHEDULES"), &schedules);

        env.events().publish(
            (symbol_short!("schedule"), ScheduleEvent::Executed),
            (schedule_id, current_time),
        );

        true
    }

    /// Pause a scheduled remittance
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the schedule owner)
    /// * `schedule_id` - ID of the schedule to pause
    pub fn pause_schedule(env: Env, caller: Address, schedule_id: u32) -> bool {
        caller.require_auth();

        Self::extend_instance_ttl(&env);

        let mut schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let mut schedule = schedules.get(schedule_id).expect("Schedule not found");

        if schedule.owner != caller {
            panic!("Only the schedule owner can pause it");
        }

        schedule.active = false;
        schedules.set(schedule_id, schedule);
        env.storage()
            .instance()
            .set(&symbol_short!("SCHEDULES"), &schedules);

        env.events().publish(
            (symbol_short!("schedule"), ScheduleEvent::Paused),
            (schedule_id, caller),
        );

        true
    }

    /// Resume a paused scheduled remittance
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the schedule owner)
    /// * `schedule_id` - ID of the schedule to resume
    pub fn resume_schedule(env: Env, caller: Address, schedule_id: u32) -> bool {
        caller.require_auth();

        Self::extend_instance_ttl(&env);

        let mut schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let mut schedule = schedules.get(schedule_id).expect("Schedule not found");

        if schedule.owner != caller {
            panic!("Only the schedule owner can resume it");
        }

        schedule.active = true;
        schedules.set(schedule_id, schedule);
        env.storage()
            .instance()
            .set(&symbol_short!("SCHEDULES"), &schedules);

        env.events().publish(
            (symbol_short!("schedule"), ScheduleEvent::Resumed),
            (schedule_id, caller),
        );

        true
    }

    /// Modify schedule parameters
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the schedule owner)
    /// * `schedule_id` - ID of the schedule to modify
    /// * `amount` - New amount (None to keep current)
    /// * `frequency` - New frequency (None to keep current)
    /// * `frequency_days` - New frequency days (None to keep current)
    /// * `end_timestamp` - New end timestamp (None to keep current)
    pub fn modify_schedule(
        env: Env,
        caller: Address,
        schedule_id: u32,
        amount: Option<i128>,
        frequency: Option<ScheduleFrequency>,
        frequency_days: Option<u32>,
        end_timestamp: Option<Option<u64>>,
    ) -> bool {
        caller.require_auth();

        Self::extend_instance_ttl(&env);

        let mut schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let mut schedule = schedules.get(schedule_id).expect("Schedule not found");

        if schedule.owner != caller {
            panic!("Only the schedule owner can modify it");
        }

        if let Some(new_amount) = amount {
            if new_amount <= 0 {
                panic!("Amount must be positive");
            }
            schedule.amount = new_amount;
        }

        if let Some(new_frequency) = frequency {
            schedule.frequency = new_frequency;
            if new_frequency == ScheduleFrequency::Custom {
                if let Some(days) = frequency_days {
                    if days == 0 {
                        panic!("Custom frequency requires frequency_days > 0");
                    }
                    schedule.frequency_days = days;
                }
            }
        } else if let Some(days) = frequency_days {
            if schedule.frequency == ScheduleFrequency::Custom {
                if days == 0 {
                    panic!("Custom frequency requires frequency_days > 0");
                }
                schedule.frequency_days = days;
            }
        }

        if let Some(new_end) = end_timestamp {
            if let Some(end) = new_end {
                if end <= schedule.start_timestamp {
                    panic!("End timestamp must be after start timestamp");
                }
            }
            schedule.end_timestamp = new_end;
        }

        schedules.set(schedule_id, schedule);
        env.storage()
            .instance()
            .set(&symbol_short!("SCHEDULES"), &schedules);

        env.events().publish(
            (symbol_short!("schedule"), ScheduleEvent::Modified),
            (schedule_id, caller),
        );

        true
    }

    /// Cancel a scheduled remittance
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the schedule owner)
    /// * `schedule_id` - ID of the schedule to cancel
    pub fn cancel_schedule(env: Env, caller: Address, schedule_id: u32) -> bool {
        caller.require_auth();

        Self::extend_instance_ttl(&env);

        let mut schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let schedule = schedules.get(schedule_id).expect("Schedule not found");

        if schedule.owner != caller {
            panic!("Only the schedule owner can cancel it");
        }

        schedules.remove(schedule_id);
        env.storage()
            .instance()
            .set(&symbol_short!("SCHEDULES"), &schedules);

        env.events().publish(
            (symbol_short!("schedule"), ScheduleEvent::Cancelled),
            (schedule_id, caller),
        );

        true
    }

    /// Get a schedule by ID
    pub fn get_schedule(env: Env, schedule_id: u32) -> Option<RemittanceSchedule> {
        let schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        schedules.get(schedule_id)
    }

    /// Get all schedules for an owner
    pub fn get_schedules(env: Env, owner: Address) -> Vec<RemittanceSchedule> {
        let schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let mut result = Vec::new(&env);
        let max_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32);

        for i in 1..=max_id {
            if let Some(schedule) = schedules.get(i) {
                if schedule.owner == owner {
                    result.push_back(schedule);
                }
            }
        }
        result
    }

    /// Get all schedules ready for execution
    pub fn get_ready_schedules(env: Env) -> Vec<RemittanceSchedule> {
        let current_time = env.ledger().timestamp();
        let schedules: Map<u32, RemittanceSchedule> = env
            .storage()
            .instance()
            .get(&symbol_short!("SCHEDULES"))
            .unwrap_or_else(|| Map::new(&env));

        let mut result = Vec::new(&env);
        let max_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32);

        for i in 1..=max_id {
            if let Some(schedule) = schedules.get(i) {
                if schedule.active
                    && schedule.next_execution <= current_time
                    && (schedule.end_timestamp.is_none()
                        || schedule.end_timestamp.unwrap() >= current_time)
                {
                    result.push_back(schedule);
                }
            }
        }
        result
    }

    /// Validate schedule parameters
    pub fn validate_schedule(
        env: Env,
        amount: i128,
        frequency: ScheduleFrequency,
        frequency_days: u32,
        start_timestamp: u64,
        end_timestamp: Option<u64>,
    ) -> bool {
        if amount <= 0 {
            return false;
        }

        if frequency == ScheduleFrequency::Custom && frequency_days == 0 {
            return false;
        }

        let current_time = env.ledger().timestamp();
        if start_timestamp < current_time {
            return false;
        }

        if let Some(end) = end_timestamp {
            if end <= start_timestamp {
                return false;
            }
        }

        true
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
    fn test_create_schedule() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RecurringRemittance);
        let client = RecurringRemittanceClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let current_time = env.ledger().timestamp();
        let start_time = current_time + 86400; // 1 day from now

        let schedule_id = client.create_schedule(
            &owner,
            &1000i128,
            &None,
            &ScheduleFrequency::Weekly,
            &0u32,
            &start_time,
            &None,
        );

        assert!(schedule_id > 0);

        let schedule = client.get_schedule(&schedule_id).unwrap();
        assert_eq!(schedule.owner, owner);
        assert_eq!(schedule.amount, 1000);
        assert_eq!(schedule.frequency, ScheduleFrequency::Weekly);
        assert!(schedule.active);
    }

    #[test]
    fn test_pause_resume_schedule() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RecurringRemittance);
        let client = RecurringRemittanceClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let current_time = env.ledger().timestamp();
        let start_time = current_time + 86400;

        let schedule_id = client.create_schedule(
            &owner,
            &1000i128,
            &None,
            &ScheduleFrequency::Monthly,
            &0u32,
            &start_time,
            &None,
        );

        client.pause_schedule(&owner, &schedule_id);
        let schedule = client.get_schedule(&schedule_id).unwrap();
        assert!(!schedule.active);

        client.resume_schedule(&owner, &schedule_id);
        let schedule = client.get_schedule(&schedule_id).unwrap();
        assert!(schedule.active);
    }

    #[test]
    fn test_modify_schedule() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RecurringRemittance);
        let client = RecurringRemittanceClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let current_time = env.ledger().timestamp();
        let start_time = current_time + 86400;

        let schedule_id = client.create_schedule(
            &owner,
            &1000i128,
            &None,
            &ScheduleFrequency::Weekly,
            &0u32,
            &start_time,
            &None,
        );

        client.modify_schedule(
            &owner,
            &schedule_id,
            &Some(2000i128),
            &None,
            &None,
            &None,
        );

        let schedule = client.get_schedule(&schedule_id).unwrap();
        assert_eq!(schedule.amount, 2000);
    }

    #[test]
    fn test_cancel_schedule() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RecurringRemittance);
        let client = RecurringRemittanceClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let current_time = env.ledger().timestamp();
        let start_time = current_time + 86400;

        let schedule_id = client.create_schedule(
            &owner,
            &1000i128,
            &None,
            &ScheduleFrequency::Weekly,
            &0u32,
            &start_time,
            &None,
        );

        client.cancel_schedule(&owner, &schedule_id);
        let schedule = client.get_schedule(&schedule_id);
        assert!(schedule.is_none());
    }
}

