#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Map, Symbol, Vec,
};

// Storage TTL constants
const INSTANCE_LIFETIME_THRESHOLD: u32 = 17280; // ~1 day
const INSTANCE_BUMP_AMOUNT: u32 = 518400; // ~30 days

/// Time window for rate limiting
#[derive(Clone, Copy, PartialEq, Eq)]
#[contracttype]
pub enum TimeWindow {
    PerMinute = 60,
    PerHour = 3600,
    PerDay = 86400,
    Custom = 0, // Custom uses seconds
}

/// Rate limit configuration
#[derive(Clone)]
#[contracttype]
pub struct RateLimitConfig {
    pub max_calls: u32,
    pub time_window: TimeWindow,
    pub window_seconds: u64, // For custom time windows
}

/// Rate limit tracking data
#[derive(Clone)]
#[contracttype]
pub struct RateLimitTracker {
    pub calls: u32,
    pub window_start: u64,
    pub last_reset: u64,
}

/// Rate limit status
#[derive(Clone)]
#[contracttype]
pub struct RateLimitStatus {
    pub calls_remaining: u32,
    pub window_start: u64,
    pub window_end: u64,
    pub is_limited: bool,
}

#[contract]
pub struct RateLimiter;

#[contractimpl]
impl RateLimiter {
    /// Initialize rate limit configuration for a function
    ///
    /// # Arguments
    /// * `admin` - Address of the admin (must authorize)
    /// * `function_name` - Name of the function to rate limit
    /// * `max_calls` - Maximum calls allowed in the time window
    /// * `time_window` - Time window type
    /// * `window_seconds` - Custom window in seconds (if time_window is Custom)
    pub fn init_rate_limit(
        env: Env,
        admin: Address,
        function_name: Symbol,
        max_calls: u32,
        time_window: TimeWindow,
        window_seconds: u64,
    ) -> bool {
        admin.require_auth();

        if max_calls == 0 {
            panic!("Max calls must be greater than 0");
        }

        if time_window == TimeWindow::Custom && window_seconds == 0 {
            panic!("Custom time window requires window_seconds > 0");
        }

        Self::extend_instance_ttl(&env);

        let config_key = symbol_short!("CONFIG");
        let mut configs: Map<Symbol, RateLimitConfig> = env
            .storage()
            .instance()
            .get(&config_key)
            .unwrap_or_else(|| Map::new(&env));

        let window_secs = match time_window {
            TimeWindow::Custom => window_seconds,
            _ => time_window as u64,
        };

        let config = RateLimitConfig {
            max_calls,
            time_window,
            window_seconds: window_secs,
        };

        configs.set(function_name, config);
        env.storage().instance().set(&config_key, &configs);

        true
    }

    /// Check and record a function call for rate limiting
    ///
    /// # Arguments
    /// * `caller` - Address making the call
    /// * `function_name` - Name of the function being called
    ///
    /// # Returns
    /// True if call is allowed, panics if rate limit exceeded
    pub fn check_rate_limit(env: Env, caller: Address, function_name: Symbol) -> bool {
        Self::extend_instance_ttl(&env);

        let config_key = symbol_short!("CONFIG");
        let configs: Map<Symbol, RateLimitConfig> = env
            .storage()
            .instance()
            .get(&config_key)
            .unwrap_or_else(|| Map::new(&env));

        let config = configs.get(function_name.clone()).expect("Rate limit not configured");

        // Check whitelist
        let whitelist_key = symbol_short!("WHITELIST");
        let whitelist: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&whitelist_key)
            .unwrap_or_else(|| Map::new(&env));

        if whitelist.get(caller.clone()).unwrap_or(false) {
            return true; // Whitelisted addresses bypass rate limits
        }

        let tracker_key = symbol_short!("TRACKER");
        let mut trackers: Map<(Address, Symbol), RateLimitTracker> = env
            .storage()
            .instance()
            .get(&tracker_key)
            .unwrap_or_else(|| Map::new(&env));

        let current_time = env.ledger().timestamp();
        let window_seconds = match config.time_window {
            TimeWindow::Custom => config.window_seconds,
            _ => config.time_window as u64,
        };

        let tracker_key_tuple = (caller.clone(), function_name.clone());
        let mut tracker = trackers
            .get(tracker_key_tuple.clone())
            .unwrap_or_else(|| RateLimitTracker {
                calls: 0,
                window_start: current_time,
                last_reset: current_time,
            });

        // Reset window if it has expired
        if current_time >= tracker.window_start + window_seconds {
            tracker.calls = 0;
            tracker.window_start = current_time;
            tracker.last_reset = current_time;
        }

        // Check if rate limit exceeded
        if tracker.calls >= config.max_calls {
            panic!("Rate limit exceeded for function");
        }

        // Increment call count
        tracker.calls += 1;
        trackers.set(tracker_key_tuple.clone(), tracker);
        env.storage().instance().set(&tracker_key, &trackers);

        true
    }

    /// Get rate limit status for an address and function
    ///
    /// # Arguments
    /// * `caller` - Address to check
    /// * `function_name` - Function name to check
    ///
    /// # Returns
    /// RateLimitStatus with current status
    pub fn get_rate_limit_status(env: Env, caller: Address, function_name: Symbol) -> RateLimitStatus {
        let config_key = symbol_short!("CONFIG");
        let configs: Map<Symbol, RateLimitConfig> = env
            .storage()
            .instance()
            .get(&config_key)
            .unwrap_or_else(|| Map::new(&env));

        let config = configs.get(function_name.clone()).expect("Rate limit not configured");

        let tracker_key = symbol_short!("TRACKER");
        let trackers: Map<(Address, Symbol), RateLimitTracker> = env
            .storage()
            .instance()
            .get(&tracker_key)
            .unwrap_or_else(|| Map::new(&env));

        let current_time = env.ledger().timestamp();
        let window_seconds = match config.time_window {
            TimeWindow::Custom => config.window_seconds,
            _ => config.time_window as u64,
        };

        let tracker_key_tuple = (caller.clone(), function_name.clone());
        let tracker = trackers
            .get(tracker_key_tuple)
            .unwrap_or_else(|| RateLimitTracker {
                calls: 0,
                window_start: current_time,
                last_reset: current_time,
            });

        let window_end = tracker.window_start + window_seconds;
        let calls_remaining = if tracker.calls >= config.max_calls {
            0
        } else {
            config.max_calls - tracker.calls
        };

        RateLimitStatus {
            calls_remaining,
            window_start: tracker.window_start,
            window_end,
            is_limited: tracker.calls >= config.max_calls,
        }
    }

    /// Add address to whitelist
    ///
    /// # Arguments
    /// * `admin` - Address of the admin (must authorize)
    /// * `address` - Address to whitelist
    pub fn add_to_whitelist(env: Env, admin: Address, address: Address) -> bool {
        admin.require_auth();

        Self::extend_instance_ttl(&env);

        let whitelist_key = symbol_short!("WHITELIST");
        let mut whitelist: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&whitelist_key)
            .unwrap_or_else(|| Map::new(&env));

        whitelist.set(address, true);
        env.storage().instance().set(&whitelist_key, &whitelist);

        true
    }

    /// Remove address from whitelist
    ///
    /// # Arguments
    /// * `admin` - Address of the admin (must authorize)
    /// * `address` - Address to remove from whitelist
    pub fn remove_from_whitelist(env: Env, admin: Address, address: Address) -> bool {
        admin.require_auth();

        Self::extend_instance_ttl(&env);

        let whitelist_key = symbol_short!("WHITELIST");
        let mut whitelist: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&whitelist_key)
            .unwrap_or_else(|| Map::new(&env));

        whitelist.remove(address);
        env.storage().instance().set(&whitelist_key, &whitelist);

        true
    }

    /// Reset rate limit for an address and function (admin only)
    ///
    /// # Arguments
    /// * `admin` - Address of the admin (must authorize)
    /// * `caller` - Address whose rate limit to reset
    /// * `function_name` - Function name
    pub fn reset_rate_limit(
        env: Env,
        admin: Address,
        caller: Address,
        function_name: Symbol,
    ) -> bool {
        admin.require_auth();

        Self::extend_instance_ttl(&env);

        let tracker_key = symbol_short!("TRACKER");
        let mut trackers: Map<(Address, Symbol), RateLimitTracker> = env
            .storage()
            .instance()
            .get(&tracker_key)
            .unwrap_or_else(|| Map::new(&env));

        let current_time = env.ledger().timestamp();
        let tracker_key_tuple = (caller, function_name);

        let new_tracker = RateLimitTracker {
            calls: 0,
            window_start: current_time,
            last_reset: current_time,
        };

        trackers.set(tracker_key_tuple, new_tracker);
        env.storage().instance().set(&tracker_key, &trackers);

        true
    }

    /// Check if address is whitelisted
    ///
    /// # Arguments
    /// * `address` - Address to check
    ///
    /// # Returns
    /// True if address is whitelisted
    pub fn is_whitelisted(env: Env, address: Address) -> bool {
        let whitelist_key = symbol_short!("WHITELIST");
        let whitelist: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&whitelist_key)
            .unwrap_or_else(|| Map::new(&env));

        whitelist.get(address).unwrap_or(false)
    }

    /// Get all whitelisted addresses
    ///
    /// # Returns
    /// Vec of whitelisted addresses
    pub fn get_whitelist(env: Env) -> Vec<Address> {
        let _whitelist_key = symbol_short!("WHITELIST");
        // Note: Map doesn't have an iterator, so we'd need to track addresses separately
        // For now, return empty vec - in production, maintain a separate list
        Vec::new(&env)
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
    fn test_init_rate_limit() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RateLimiter);
        let client = RateLimiterClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let function_name = symbol_short!("test_func");

        let result = client.init_rate_limit(
            &admin,
            &function_name,
            &10u32,
            &TimeWindow::PerMinute,
            &0u64,
        );

        assert!(result);
    }

    #[test]
    #[should_panic(expected = "Rate limit exceeded")]
    fn test_check_rate_limit() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RateLimiter);
        let client = RateLimiterClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let caller = Address::generate(&env);
        let function_name = symbol_short!("test_func");

        client.init_rate_limit(
            &admin,
            &function_name,
            &5u32,
            &TimeWindow::PerMinute,
            &0u64,
        );

        // Make 5 calls (should succeed)
        for _ in 0..5 {
            let result = client.check_rate_limit(&caller, &function_name);
            assert!(result);
        }

        // 6th call should panic
        client.check_rate_limit(&caller, &function_name);
    }

    #[test]
    fn test_whitelist() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RateLimiter);
        let client = RateLimiterClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let whitelisted = Address::generate(&env);
        let function_name = symbol_short!("test_func");

        client.init_rate_limit(
            &admin,
            &function_name,
            &1u32,
            &TimeWindow::PerMinute,
            &0u64,
        );

        client.add_to_whitelist(&admin, &whitelisted);

        // Whitelisted address should bypass rate limit
        let result1 = client.check_rate_limit(&whitelisted, &function_name);
        assert!(result1);

        let result2 = client.check_rate_limit(&whitelisted, &function_name);
        assert!(result2); // Should still work even though limit is 1
    }

    #[test]
    fn test_get_rate_limit_status() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, RateLimiter);
        let client = RateLimiterClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let caller = Address::generate(&env);
        let function_name = symbol_short!("test_func");

        client.init_rate_limit(
            &admin,
            &function_name,
            &10u32,
            &TimeWindow::PerMinute,
            &0u64,
        );

        let status = client.get_rate_limit_status(&caller, &function_name);
        assert_eq!(status.calls_remaining, 10);
        assert!(!status.is_limited);
    }
}

