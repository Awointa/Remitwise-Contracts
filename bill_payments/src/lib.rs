#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Env, Map, String, Symbol, Vec,
};

// Event topics
const BILL_CREATED: Symbol = symbol_short!("created");
const BILL_PAID: Symbol = symbol_short!("paid");
const RECURRING_BILL_CREATED: Symbol = symbol_short!("recurring");

// Event data structures
#[derive(Clone)]
#[contracttype]
pub struct BillCreatedEvent {
    pub bill_id: u32,
    pub name: String,
    pub amount: i128,
    pub due_date: u64,
    pub recurring: bool,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct BillPaidEvent {
    pub bill_id: u32,
    pub name: String,
    pub amount: i128,
    pub timestamp: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct RecurringBillCreatedEvent {
    pub bill_id: u32,
    pub parent_bill_id: u32,
    pub name: String,
    pub amount: i128,
    pub due_date: u64,
    pub timestamp: u64,
}
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Map, String,
    Vec,
};

// Storage TTL constants
const INSTANCE_LIFETIME_THRESHOLD: u32 = 17280; // ~1 day
const INSTANCE_BUMP_AMOUNT: u32 = 518400; // ~30 days

/// Bill data structure with owner tracking for access control
#[derive(Clone)]
#[contracttype]
pub struct Bill {
    pub id: u32,
    pub owner: Address,
    pub name: String,
    pub amount: i128,
    pub due_date: u64,
    pub recurring: bool,
    pub frequency_days: u32,
    pub paid: bool,
    pub created_at: u64,
    pub paid_at: Option<u64>,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    BillNotFound = 1,
    BillAlreadyPaid = 2,
    InvalidAmount = 3,
    InvalidFrequency = 4,
    Unauthorized = 5,
}

/// Events emitted by the contract for audit trail
#[contracttype]
#[derive(Clone)]
pub enum BillEvent {
    Created,
    Paid,
}

#[contract]
pub struct BillPayments;

#[contractimpl]
impl BillPayments {
    /// Create a new bill
    ///
    /// # Arguments
    /// * `owner` - Address of the bill owner (must authorize)
    /// * `name` - Name of the bill (e.g., "Electricity", "School Fees")
    /// * `amount` - Amount to pay (must be positive)
    /// * `due_date` - Due date as Unix timestamp
    /// * `recurring` - Whether this is a recurring bill
    /// * `frequency_days` - Frequency in days for recurring bills (must be > 0 if recurring)
    ///
    /// # Returns
    /// The ID of the created bill
    ///
    /// # Errors
    /// * `InvalidAmount` - If amount is zero or negative
    /// * `InvalidFrequency` - If recurring is true but frequency_days is 0
    pub fn create_bill(
        env: Env,
        owner: Address,
        name: String,
        amount: i128,
        due_date: u64,
        recurring: bool,
        frequency_days: u32,
    ) -> Result<u32, Error> {
        // Access control: require owner authorization
        owner.require_auth();

        // Validate inputs
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        if recurring && frequency_days == 0 {
            return Err(Error::InvalidFrequency);
        }

        // Extend storage TTL
        Self::extend_instance_ttl(&env);
        let mut bills: Map<u32, Bill> = env
            .storage()
            .instance()
            .get(&symbol_short!("BILLS"))
            .unwrap_or_else(|| Map::new(&env));

        let next_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32)
            + 1;

        let current_time = env.ledger().timestamp();
        let bill = Bill {
            id: next_id,
            owner: owner.clone(),
            name: name.clone(),
            amount,
            due_date,
            recurring,
            frequency_days,
            paid: false,
            created_at: current_time,
            paid_at: None,
        };

        let bill_owner = bill.owner.clone();
        bills.set(next_id, bill);
        env.storage()
            .instance()
            .set(&symbol_short!("BILLS"), &bills);
        env.storage()
            .instance()
            .set(&symbol_short!("NEXT_ID"), &next_id);

        // Emit BillCreated event
        let event = BillCreatedEvent {
            bill_id: next_id,
            name: name.clone(),
            amount,
            due_date,
            recurring,
            timestamp: env.ledger().timestamp(),
        };
        env.events().publish((BILL_CREATED,), event);

        next_id
        // Emit event for audit trail
        env.events().publish(
            (symbol_short!("bill"), BillEvent::Created),
            (next_id, bill_owner),
        );

        Ok(next_id)
    }

    /// Mark a bill as paid
    ///
    /// # Arguments
    /// * `caller` - Address of the caller (must be the bill owner)
    /// * `bill_id` - ID of the bill
    ///
    /// # Returns
    /// Ok(()) if payment was successful
    ///
    /// # Errors
    /// * `BillNotFound` - If bill with given ID doesn't exist
    /// * `BillAlreadyPaid` - If bill is already marked as paid
    /// * `Unauthorized` - If caller is not the bill owner
    pub fn pay_bill(env: Env, caller: Address, bill_id: u32) -> Result<(), Error> {
        // Access control: require caller authorization
        caller.require_auth();

        // Extend storage TTL
        Self::extend_instance_ttl(&env);
        let mut bills: Map<u32, Bill> = env
            .storage()
            .instance()
            .get(&symbol_short!("BILLS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut bill = bills.get(bill_id).ok_or(Error::BillNotFound)?;

            bill.paid = true;

            // Emit BillPaid event
            let paid_event = BillPaidEvent {
                bill_id,
                name: bill.name.clone(),
                amount: bill.amount,
                timestamp: env.ledger().timestamp(),
            };
            env.events().publish((BILL_PAID,), paid_event);

            // If recurring, create next bill
            if bill.recurring {
                let next_due_date = bill.due_date + (bill.frequency_days as u64 * 86400);
                let next_bill = Bill {
                    id: env
                        .storage()
                        .instance()
                        .get(&symbol_short!("NEXT_ID"))
                        .unwrap_or(0u32)
                        + 1,
                    name: bill.name.clone(),
                    amount: bill.amount,
                    due_date: next_due_date,
                    recurring: true,
                    frequency_days: bill.frequency_days,
                    paid: false,
                };

                let next_id = next_bill.id;

                // Emit RecurringBillCreated event
                let recurring_event = RecurringBillCreatedEvent {
                    bill_id: next_id,
                    parent_bill_id: bill_id,
                    name: bill.name.clone(),
                    amount: bill.amount,
                    due_date: next_due_date,
                    timestamp: env.ledger().timestamp(),
                };
                env.events()
                    .publish((RECURRING_BILL_CREATED,), recurring_event);

                bills.set(next_id, next_bill);
                env.storage()
                    .instance()
                    .set(&symbol_short!("NEXT_ID"), &next_id);
            }
        // Access control: verify caller is the owner
        if bill.owner != caller {
            return Err(Error::Unauthorized);
        }

        if bill.paid {
            return Err(Error::BillAlreadyPaid);
        }

        let current_time = env.ledger().timestamp();
        bill.paid = true;
        bill.paid_at = Some(current_time);

        // If recurring, create next bill
        if bill.recurring {
            let next_due_date = bill.due_date + (bill.frequency_days as u64 * 86400);
            let next_id = env
                .storage()
                .instance()
                .get(&symbol_short!("NEXT_ID"))
                .unwrap_or(0u32)
                + 1;

            let next_bill = Bill {
                id: next_id,
                owner: bill.owner.clone(),
                name: bill.name.clone(),
                amount: bill.amount,
                due_date: next_due_date,
                recurring: true,
                frequency_days: bill.frequency_days,
                paid: false,
                created_at: current_time,
                paid_at: None,
            };
            bills.set(next_id, next_bill);
            env.storage()
                .instance()
                .set(&symbol_short!("NEXT_ID"), &next_id);
        }

        bills.set(bill_id, bill);
        env.storage()
            .instance()
            .set(&symbol_short!("BILLS"), &bills);

        // Emit event for audit trail
        env.events()
            .publish((symbol_short!("bill"), BillEvent::Paid), (bill_id, caller));

        Ok(())
    }

    /// Get a bill by ID
    ///
    /// # Arguments
    /// * `bill_id` - ID of the bill
    ///
    /// # Returns
    /// Bill struct or None if not found
    pub fn get_bill(env: Env, bill_id: u32) -> Option<Bill> {
        let bills: Map<u32, Bill> = env
            .storage()
            .instance()
            .get(&symbol_short!("BILLS"))
            .unwrap_or_else(|| Map::new(&env));

        bills.get(bill_id)
    }

    /// Get all unpaid bills for a specific owner
    ///
    /// # Arguments
    /// * `owner` - Address of the bill owner
    ///
    /// # Returns
    /// Vec of unpaid Bill structs belonging to the owner
    pub fn get_unpaid_bills(env: Env, owner: Address) -> Vec<Bill> {
        let bills: Map<u32, Bill> = env
            .storage()
            .instance()
            .get(&symbol_short!("BILLS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut result = Vec::new(&env);
        let max_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32);

        for i in 1..=max_id {
            if let Some(bill) = bills.get(i) {
                if !bill.paid && bill.owner == owner {
                    result.push_back(bill);
                }
            }
        }
        result
    }

    /// Get all overdue unpaid bills
    ///
    /// # Returns
    /// Vec of unpaid bills that are past their due date
    pub fn get_overdue_bills(env: Env) -> Vec<Bill> {
        let current_time = env.ledger().timestamp();
        let bills: Map<u32, Bill> = env
            .storage()
            .instance()
            .get(&symbol_short!("BILLS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut result = Vec::new(&env);
        let max_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32);

        for i in 1..=max_id {
            if let Some(bill) = bills.get(i) {
                if !bill.paid && bill.due_date < current_time {
                    result.push_back(bill);
                }
            }
        }
        result
    }

    /// Get total amount of unpaid bills for a specific owner
    ///
    /// # Arguments
    /// * `owner` - Address of the bill owner
    ///
    /// # Returns
    /// Total amount of all unpaid bills belonging to the owner
    pub fn get_total_unpaid(env: Env, owner: Address) -> i128 {
        let unpaid = Self::get_unpaid_bills(env, owner);
        let mut total = 0i128;
        for bill in unpaid.iter() {
            total += bill.amount;
        }
        total
    }

    /// Cancel/delete a bill
    ///
    /// # Arguments
    /// * `bill_id` - ID of the bill to cancel
    ///
    /// # Returns
    /// Ok(()) if cancellation was successful
    ///
    /// # Errors
    /// * `BillNotFound` - If bill with given ID doesn't exist
    pub fn cancel_bill(env: Env, bill_id: u32) -> Result<(), Error> {
        let mut bills: Map<u32, Bill> = env
            .storage()
            .instance()
            .get(&symbol_short!("BILLS"))
            .unwrap_or_else(|| Map::new(&env));

        if bills.get(bill_id).is_none() {
            return Err(Error::BillNotFound);
        }

        bills.remove(bill_id);
        env.storage()
            .instance()
            .set(&symbol_short!("BILLS"), &bills);

        Ok(())
    }

    /// Get all bills (paid and unpaid)
    ///
    /// # Returns
    /// Vec of all Bill structs
    pub fn get_all_bills(env: Env) -> Vec<Bill> {
        let bills: Map<u32, Bill> = env
            .storage()
            .instance()
            .get(&symbol_short!("BILLS"))
            .unwrap_or_else(|| Map::new(&env));

        let mut result = Vec::new(&env);
        let max_id = env
            .storage()
            .instance()
            .get(&symbol_short!("NEXT_ID"))
            .unwrap_or(0u32);

        for i in 1..=max_id {
            if let Some(bill) = bills.get(i) {
                result.push_back(bill);
            }
        }
        result
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
    fn test_create_bill_emits_event() {
        let env = Env::default();
        let contract_id = env.register_contract(None, BillPayments);
        let client = BillPaymentsClient::new(&env, &contract_id);

        // Create a bill
        let bill_id = client.create_bill(
            &String::from_str(&env, "Electricity"),
            &500,
            &1735689600,
            &false,
            &0,
        );
        assert_eq!(bill_id, 1);

        // Verify event was emitted
        let events = env.events().all();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_pay_bill_emits_event() {
        let env = Env::default();
        let contract_id = env.register_contract(None, BillPayments);
        let client = BillPaymentsClient::new(&env, &contract_id);

        // Create a bill
        let bill_id = client.create_bill(
            &String::from_str(&env, "Water Bill"),
            &300,
            &1735689600,
            &false,
            &0,
        );

        // Get events before paying
        let events_before = env.events().all().len();

        // Pay the bill
        let result = client.pay_bill(&bill_id);
        assert!(result);

        // Verify BillPaid event was emitted (1 new event)
        let events_after = env.events().all().len();
        assert_eq!(events_after - events_before, 1);
    }

    #[test]
    fn test_pay_recurring_bill_emits_multiple_events() {
        let env = Env::default();
        let contract_id = env.register_contract(None, BillPayments);
        let client = BillPaymentsClient::new(&env, &contract_id);

        // Create a recurring bill
        let bill_id = client.create_bill(
            &String::from_str(&env, "Rent"),
            &1000,
            &1735689600,
            &true,
            &30, // Monthly
        );

        // Get events before paying
        let events_before = env.events().all().len();

        // Pay the recurring bill
        client.pay_bill(&bill_id);

        // Should emit BillPaid and RecurringBillCreated events (2 new events)
        let events_after = env.events().all().len();
        assert_eq!(events_after - events_before, 2);
    }

    #[test]
    fn test_multiple_bills_emit_separate_events() {
        let env = Env::default();
        let contract_id = env.register_contract(None, BillPayments);
        let client = BillPaymentsClient::new(&env, &contract_id);

        // Create multiple bills
        client.create_bill(
            &String::from_str(&env, "Bill 1"),
            &100,
            &1735689600,
            &false,
            &0,
        );
        client.create_bill(
            &String::from_str(&env, "Bill 2"),
            &200,
            &1735689600,
            &false,
            &0,
        );
        client.create_bill(
            &String::from_str(&env, "Bill 3"),
            &300,
            &1735689600,
            &true,
            &30,
        );

        // Should have 3 BillCreated events
        let events = env.events().all();
        assert_eq!(events.len(), 3);
    }
}
mod test;
