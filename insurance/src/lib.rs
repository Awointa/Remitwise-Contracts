// Inside your insurance contract implementation
pub fn execute_due_premium_schedules(env: Env, policy_id: BytesN<32>) {
    // 1. Authorization: Ensure the invoker has permission
    let policy = get_policy(&env, &policy_id);
    policy.owner.require_auth();

    // 2. Idempotency Check: 
    // Check if the current time is actually past the next_payment_date
    let current_time = env.ledger().timestamp();
    if current_time < policy.next_payment_date {
        panic!("Premium not yet due: Idempotency protection triggered.");
    }

    // 3. Process Payment Logic (Interacting with Stellar Asset Contract)
    // ... transfer logic here ...

    // 4. Update State: Move the next_payment_date forward (e.g., 30 days)
    let mut updated_policy = policy;
    updated_policy.next_payment_date += SECONDS_IN_MONTH; 
    updated_policy.last_payment_timestamp = current_time;
    
    put_policy(&env, &policy_id, &updated_policy);

    // 5. Emit Event with external_ref for off-chain tracking
    env.events().publish(
        (Symbol::new(&env, "insurance"), Symbol::new(&env, "paid")),
        PremiumPaidEvent { policy_id, amount: updated_policy.monthly_premium, ... }
    );
}