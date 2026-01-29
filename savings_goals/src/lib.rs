#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Env, Map, String, Symbol, Vec,
};

// Event topics
const GOAL_CREATED: Symbol = symbol_short!("created");
const FUNDS_ADDED: Symbol = symbol_short!("added");
const GOAL_COMPLETED: Symbol = symbol_short!("completed");

// Event data structures
#[derive(Clone)]
#[contracttype]
pub struct GoalCreatedEvent {
    pub goal_id: u32,
    pub name: String,
    pub target_amount: i128,
    pub target_date: u64,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct FundsAddedEvent {
    pub goal_id: u32,
    pub amount: i128,
    pub new_total: i128,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct GoalCompletedEvent {
    pub goal_id: u32,
    pub name: String,
    pub final_amount: i128,
    pub timestamp: u64,
}
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Map, String, Symbol, Vec,
};

// Storage TTL constants
const INSTANCE_LIFETIME_THRESHOLD: u32 = 17280; // ~1 day
const INSTANCE_BUMP_AMOUNT: u32 = 518400; // ~30 days

/// Savings goal data structure with owner tracking for access control
#[contract]
pub struct SavingsGoalContract;

#[contracttype]
#[derive(Clone)]
pub struct SavingsGoal {
    pub id: u32,
    pub owner: Address,
    pub name: String,
    pub target_amount: i128,
    pub current_amount: i128,
    pub target_date: u64,
    pub locked: bool,
}

/// Events emitted by the contract for audit trail
#[contracttype]
#[derive(Clone)]
pub enum SavingsEvent {
    GoalCreated,
    FundsAdded,
    FundsWithdrawn,
    GoalCompleted,
    GoalLocked,
    GoalUnlocked,
}

#[contractimpl]
impl SavingsGoalContract {
    // Storage keys
    const STORAGE_NEXT_ID: Symbol = symbol_short!("NEXT_ID");
    const STORAGE_GOALS: Symbol = symbol_short!("GOALS");

    /// Initialize contract storage
    pub fn init(env: Env) {
        let storage = env.storage().persistent();

        if storage.get::<_, u32>(&Self::STORAGE_NEXT_ID).is_none() {
            storage.set(&Self::STORAGE_NEXT_ID, &1u32);
        }

        if storage
            .get::<_, Map<u32, SavingsGoal>>(&Self::STORAGE_GOALS)
            .is_none()
        {
            storage.set(&Self::STORAGE_GOALS, &Map::<u32, SavingsGoal>::new(&env));
        }
    }

    /// Create a new savings goal
    ///
    /// # Arguments
    /// * `owner` - Address of the goal owner (must authorize)
    /// * `name` - Name of the goal (e.g., "Education", "Medical")
    /// * `target_amount` - Target amount to save (must be positive)
    /// * `target_date` - Target date as Unix timestamp
    ///
    /// # Returns
    /// The ID of the created goal
    ///
    /// # Panics
    /// - If owner doesn't authorize the transaction
    /// - If target_amount is not positive
    pub fn create_goal(
        env: Env,
        owner: Address,
        name: String,
        target_amount: i128,
        target_date: u64,
    ) -> u32 {
        // Access control: require owner authorization
        owner.require_auth();

        // Input validation
        if target_amount <= 0 {
            panic!("Target amount must be positive");
        }

        // Extend storage TTL
        Self::extend_instance_ttl(&env);

        let mut goals: Map<u32, SavingsGoal> = env
            .storage()
            .instance()
            .get(&symbol_short!("GOALS"))
            .unwrap_or_else(|| Map::new(&env));

        let next_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32)
            + 1;

        let goal = SavingsGoal {
            id: next_id,
            owner: owner.clone(),
            name,
            target_amount,
            current_amount: 0,
            target_date,
            locked: true,
        };

        goals.set(next_id, goal);
        env.storage()
            .instance()
            .set(&symbol_short!("GOALS"), &goals);
        env.storage()
            .instance()
            .set(&symbol_short!("NEXT_ID"), &next_id);

        // Emit GoalCreated event
        let event = GoalCreatedEvent {
            goal_id: next_id,
            name: name.clone(),
            target_amount,
            target_date,
            timestamp: env.ledger().timestamp(),
        };
        env.events().publish((GOAL_CREATED,), event);
        // Emit event for audit trail
        env.events().publish(
            (symbol_short!("savings"), SavingsEvent::GoalCreated),
            (next_id, owner),
        );

        next_id
    }

    /// Add funds to a savings goal
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the goal owner)
    /// * `goal_id` - ID of the goal
    /// * `amount` - Amount to add (must be positive)
    ///
    /// # Returns
    /// Updated current amount
    ///
    /// # Panics
    /// - If caller is not the goal owner
    /// - If goal is not found
    /// - If amount is not positive
    pub fn add_to_goal(env: Env, caller: Address, goal_id: u32, amount: i128) -> i128 {
        // Access control: require caller authorization
        caller.require_auth();

        // Input validation
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        // Extend storage TTL
        Self::extend_instance_ttl(&env);

        let mut goals: Map<u32, SavingsGoal> = env
            .storage()
            .instance()
            .get(&symbol_short!("GOALS"))
            .unwrap_or_else(|| Map::new(&env));

        if let Some(mut goal) = goals.get(goal_id) {
            goal.current_amount += amount;
            let new_total = goal.current_amount;
            let was_completed = goal.current_amount >= goal.target_amount;

            goals.set(goal_id, goal.clone());
            env.storage()
                .instance()
                .set(&symbol_short!("GOALS"), &goals);

            // Emit FundsAdded event
            let funds_event = FundsAddedEvent {
                goal_id,
                amount,
                new_total,
                timestamp: env.ledger().timestamp(),
            };
            env.events().publish((FUNDS_ADDED,), funds_event);

            // Emit GoalCompleted event if goal just reached target
            if was_completed && (new_total - amount) < goal.target_amount {
                let completed_event = GoalCompletedEvent {
                    goal_id,
                    name: goal.name.clone(),
                    final_amount: new_total,
                    timestamp: env.ledger().timestamp(),
                };
                env.events().publish((GOAL_COMPLETED,), completed_event);
            }

            goal.current_amount
        } else {
            -1 // Goal not found
        let mut goal = goals.get(goal_id).expect("Goal not found");

        // Access control: verify caller is the owner
        if goal.owner != caller {
            panic!("Only the goal owner can add funds");
        }

        goal.current_amount += amount;
        let new_amount = goal.current_amount;
        let is_completed = goal.current_amount >= goal.target_amount;
        let goal_owner = goal.owner.clone();

        goals.set(goal_id, goal);
        env.storage()
            .instance()
            .set(&symbol_short!("GOALS"), &goals);

        // Emit event for audit trail
        env.events().publish(
            (symbol_short!("savings"), SavingsEvent::FundsAdded),
            (goal_id, goal_owner.clone(), amount),
        );

        // Emit completion event if goal is now complete
        if is_completed {
            env.events().publish(
                (symbol_short!("savings"), SavingsEvent::GoalCompleted),
                (goal_id, goal_owner),
            );
        }

        new_amount
    }

    /// Withdraw funds from a savings goal
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the goal owner)
    /// * `goal_id` - ID of the goal
    /// * `amount` - Amount to withdraw (must be positive and <= current_amount)
    ///
    /// # Returns
    /// Updated current amount
    ///
    /// # Panics
    /// - If caller is not the goal owner
    /// - If goal is not found
    /// - If goal is locked
    /// - If amount is not positive
    /// - If amount exceeds current balance
    pub fn withdraw_from_goal(env: Env, caller: Address, goal_id: u32, amount: i128) -> i128 {
        // Access control: require caller authorization
        caller.require_auth();

        // Input validation
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        // Extend storage TTL
        Self::extend_instance_ttl(&env);

        let mut goals: Map<u32, SavingsGoal> = env
            .storage()
            .instance()
            .get(&symbol_short!("GOALS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut goal = goals.get(goal_id).expect("Goal not found");

        // Access control: verify caller is the owner
        if goal.owner != caller {
            panic!("Only the goal owner can withdraw funds");
        }

        // Check if goal is locked
        if goal.locked {
            panic!("Cannot withdraw from a locked goal");
        }

        // Check sufficient balance
        if amount > goal.current_amount {
            panic!("Insufficient balance");
        }

        goal.current_amount -= amount;
        let new_amount = goal.current_amount;

        goals.set(goal_id, goal);
        env.storage()
            .instance()
            .set(&symbol_short!("GOALS"), &goals);

        // Emit event for audit trail
        env.events().publish(
            (symbol_short!("savings"), SavingsEvent::FundsWithdrawn),
            (goal_id, caller, amount),
        );

        new_amount
    }

    /// Lock a savings goal (prevent withdrawals)
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the goal owner)
    /// * `goal_id` - ID of the goal
    ///
    /// # Panics
    /// - If caller is not the goal owner
    /// - If goal is not found
    pub fn lock_goal(env: Env, caller: Address, goal_id: u32) -> bool {
        // Access control: require caller authorization
        caller.require_auth();

        // Extend storage TTL
        Self::extend_instance_ttl(&env);

        let mut goals: Map<u32, SavingsGoal> = env
            .storage()
            .instance()
            .get(&symbol_short!("GOALS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut goal = goals.get(goal_id).expect("Goal not found");

        // Access control: verify caller is the owner
        if goal.owner != caller {
            panic!("Only the goal owner can lock this goal");
        }

        goal.locked = true;
        goals.set(goal_id, goal);
        env.storage()
            .instance()
            .set(&symbol_short!("GOALS"), &goals);

        // Emit event for audit trail
        env.events().publish(
            (symbol_short!("savings"), SavingsEvent::GoalLocked),
            (goal_id, caller),
        );

        true
    }

    /// Unlock a savings goal (allow withdrawals)
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the goal owner)
    /// * `goal_id` - ID of the goal
    ///
    /// # Panics
    /// - If caller is not the goal owner
    /// - If goal is not found
    pub fn unlock_goal(env: Env, caller: Address, goal_id: u32) -> bool {
        // Access control: require caller authorization
        caller.require_auth();

        // Extend storage TTL
        Self::extend_instance_ttl(&env);

        let mut goals: Map<u32, SavingsGoal> = env
            .storage()
            .instance()
            .get(&symbol_short!("GOALS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut goal = goals.get(goal_id).expect("Goal not found");

        // Access control: verify caller is the owner
        if goal.owner != caller {
            panic!("Only the goal owner can unlock this goal");
        }

        goal.locked = false;
        goals.set(goal_id, goal);
        env.storage()
            .instance()
            .set(&symbol_short!("GOALS"), &goals);

        // Emit event for audit trail
        env.events().publish(
            (symbol_short!("savings"), SavingsEvent::GoalUnlocked),
            (goal_id, caller),
        );

        true
    }

    /// Get a savings goal by ID
    ///
    /// # Arguments
    /// * `goal_id` - ID of the goal
    ///
    /// # Returns
    /// SavingsGoal struct or None if not found
    pub fn get_goal(env: Env, goal_id: u32) -> Option<SavingsGoal> {
        let goals: Map<u32, SavingsGoal> = env
            .storage()
            .instance()
            .get(&symbol_short!("GOALS"))
            .unwrap_or_else(|| Map::new(&env));

        goals.get(goal_id)
    }

    /// Get all savings goals for a specific owner
    ///
    /// # Arguments
    /// * `owner` - Address of the goal owner
    ///
    /// # Returns
    /// Vec of all SavingsGoal structs belonging to the owner
    pub fn get_all_goals(env: Env, owner: Address) -> Vec<SavingsGoal> {
        let goals: Map<u32, SavingsGoal> = env
            .storage()
            .instance()
            .get(&symbol_short!("GOALS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut result = Vec::new(&env);
        let max_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32);

        for i in 1..=max_id {
            if let Some(goal) = goals.get(i) {
                if goal.owner == owner {
                    result.push_back(goal);
                }
            }
        }
        result
    }

    /// Check if a goal is completed
    pub fn is_goal_completed(env: Env, goal_id: u32) -> bool {
        // Change .persistent() to .instance() to match add_to_goal
        let storage = env.storage().instance();

        let goals: Map<u32, SavingsGoal> = storage
            .get(&symbol_short!("GOALS"))
            .unwrap_or(Map::new(&env));

        if let Some(goal) = goals.get(goal_id) {
            goal.current_amount >= goal.target_amount
        } else {
            false
        }
    }

    /// Extend the TTL of instance storage
    fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Events;

    #[test]
    fn test_create_goal_emits_event() {
        let env = Env::default();
        let contract_id = env.register_contract(None, SavingsGoals);
        let client = SavingsGoalsClient::new(&env, &contract_id);

        // Create a goal
        let goal_id = client.create_goal(
            &String::from_str(&env, "Education"),
            &10000,
            &1735689600, // Future date
        );
        assert_eq!(goal_id, 1);

        // Verify event was emitted
        let events = env.events().all();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_add_to_goal_emits_event() {
        let env = Env::default();
        let contract_id = env.register_contract(None, SavingsGoals);
        let client = SavingsGoalsClient::new(&env, &contract_id);

        // Create a goal
        let goal_id = client.create_goal(&String::from_str(&env, "Medical"), &5000, &1735689600);

        // Get events before adding funds
        let events_before = env.events().all().len();

        // Add funds
        let new_amount = client.add_to_goal(&goal_id, &1000);
        assert_eq!(new_amount, 1000);

        // Verify 1 new event was emitted (FundsAdded event)
        let events_after = env.events().all().len();
        assert_eq!(events_after - events_before, 1);
    }

    #[test]
    fn test_goal_completed_emits_event() {
        let env = Env::default();
        let contract_id = env.register_contract(None, SavingsGoals);
        let client = SavingsGoalsClient::new(&env, &contract_id);

        // Create a goal with small target
        let goal_id = client.create_goal(
            &String::from_str(&env, "Emergency Fund"),
            &1000,
            &1735689600,
        );

        // Get events before adding funds
        let events_before = env.events().all().len();

        // Add funds to complete the goal
        client.add_to_goal(&goal_id, &1000);

        // Verify both FundsAdded and GoalCompleted events were emitted (2 new events)
        let events_after = env.events().all().len();
        assert_eq!(events_after - events_before, 2);
    }

    #[test]
    fn test_multiple_goals_emit_separate_events() {
        let env = Env::default();
        let contract_id = env.register_contract(None, SavingsGoals);
        let client = SavingsGoalsClient::new(&env, &contract_id);

        // Create multiple goals
        client.create_goal(&String::from_str(&env, "Goal 1"), &1000, &1735689600);
        client.create_goal(&String::from_str(&env, "Goal 2"), &2000, &1735689600);
        client.create_goal(&String::from_str(&env, "Goal 3"), &3000, &1735689600);

        // Should have 3 GoalCreated events
        let events = env.events().all();
        assert_eq!(events.len(), 3);
    }
}
mod test;
