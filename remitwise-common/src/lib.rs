#![no_std]

use soroban_sdk::{contracttype, symbol_short, Symbol};

/// Financial categories for remittance allocation
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Category {
    Spending = 1,
    Savings = 2,
    Bills = 3,
    Insurance = 4,
}

/// Family roles for access control
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FamilyRole {
    Owner = 1,
    Admin = 2,
    Member = 3,
    Viewer = 4,
}

/// Insurance coverage types
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CoverageType {
    Health = 1,
    Life = 2,
    Property = 3,
    Auto = 4,
    Liability = 5,
}

/// Event categories for logging
#[allow(dead_code)]
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum EventCategory {
    Transaction = 0,
    State = 1,
    Alert = 2,
    System = 3,
    Access = 4,
}

/// Event priorities for logging
#[allow(dead_code)]
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum EventPriority {
    Low = 0,
    Medium = 1,
    High = 2,
}

impl EventCategory {
    pub fn to_u32(self) -> u32 {
        self as u32
    }
}

impl EventPriority {
    pub fn to_u32(self) -> u32 {
        self as u32
    }
}

/// Pagination limits
pub const DEFAULT_PAGE_LIMIT: u32 = 20;
pub const MAX_PAGE_LIMIT: u32 = 50;

/// Storage TTL constants for active data
pub const INSTANCE_LIFETIME_THRESHOLD: u32 = 17280; // ~1 day
pub const INSTANCE_BUMP_AMOUNT: u32 = 518400; // ~30 days

/// Storage TTL constants for archived data
pub const ARCHIVE_LIFETIME_THRESHOLD: u32 = 17280; // ~1 day
pub const ARCHIVE_BUMP_AMOUNT: u32 = 2592000; // ~180 days (6 months)

/// Signature expiration time (24 hours in seconds)
pub const SIGNATURE_EXPIRATION: u64 = 86400;

/// Contract version
pub const CONTRACT_VERSION: u32 = 1;

/// Maximum batch size for operations
pub const MAX_BATCH_SIZE: u32 = 50;

/// Helper function to clamp limit
///
/// # Behavior Contract
///
/// `clamp_limit` normalises a caller-supplied page-size value so that every
/// pagination call in the workspace uses a consistent, bounded limit.
///
/// ## Rules (in evaluation order)
///
/// | Input condition          | Returned value        | Rationale                                      |
/// |--------------------------|----------------------|------------------------------------------------|
/// | `limit == 0`             | `DEFAULT_PAGE_LIMIT` | Zero is treated as "use the default".          |
/// | `limit > MAX_PAGE_LIMIT` | `MAX_PAGE_LIMIT`     | Cap to prevent unbounded storage reads.        |
/// | otherwise                | `limit`              | Caller value is within the valid range.        |
///
/// ## Invariants
///
/// - The return value is always in the range `[1, MAX_PAGE_LIMIT]`.
/// - `clamp_limit(0) == DEFAULT_PAGE_LIMIT` (default substitution).
/// - `clamp_limit(MAX_PAGE_LIMIT) == MAX_PAGE_LIMIT` (boundary is inclusive).
/// - `clamp_limit(MAX_PAGE_LIMIT + 1) == MAX_PAGE_LIMIT` (cap is enforced).
/// - The function is pure and has no side effects.
///
/// ## Security Assumptions
///
/// - Callers must not rely on receiving a value larger than `MAX_PAGE_LIMIT`.
/// - A zero input is **not** an error; it is silently replaced with the default.
///   Contracts that need to distinguish "no limit requested" from "default limit"
///   should inspect the raw input before calling this function.
///
/// ## Usage
///
/// ```rust
/// use remitwise_common::{clamp_limit, DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT};
///
/// assert_eq!(clamp_limit(0),                  DEFAULT_PAGE_LIMIT);
/// assert_eq!(clamp_limit(10),                 10);
/// assert_eq!(clamp_limit(MAX_PAGE_LIMIT),     MAX_PAGE_LIMIT);
/// assert_eq!(clamp_limit(MAX_PAGE_LIMIT + 1), MAX_PAGE_LIMIT);
/// ```
pub fn clamp_limit(limit: u32) -> u32 {
    if limit == 0 {
        DEFAULT_PAGE_LIMIT
    } else if limit > MAX_PAGE_LIMIT {
        MAX_PAGE_LIMIT
    } else {
        limit
    }
}

/// Event emission helper
///
/// # Deterministic topic naming
///
/// All events emitted via `RemitwiseEvents` follow a deterministic topic schema:
///
/// 1. A fixed namespace symbol: `"Remitwise"`.
/// 2. An event category as `u32` (see `EventCategory`).
/// 3. An event priority as `u32` (see `EventPriority`).
/// 4. An action `Symbol` describing the specific event or a subtype (e.g. `"created"`).
///
/// This ordering allows consumers to index and filter events reliably across contracts.
pub struct RemitwiseEvents;

impl RemitwiseEvents {
    /// Emit a single event with deterministic topics.
    ///
    /// # Parameters
    /// - `env`: Soroban environment used to publish the event.
    /// - `category`: Logical event category (`EventCategory`).
    /// - `priority`: Event priority (`EventPriority`).
    /// - `action`: A `Symbol` identifying the action or event name.
    /// - `data`: The serializable payload for the event.
    ///
    /// # Security
    /// Do not include sensitive personal data in `data` because events are publicly visible on-chain.
    pub fn emit<T>(
        env: &soroban_sdk::Env,
        category: EventCategory,
        priority: EventPriority,
        action: Symbol,
        data: T,
    ) where
        T: soroban_sdk::IntoVal<soroban_sdk::Env, soroban_sdk::Val>,
    {
        let topics = (
            symbol_short!("Remitwise"),
            category.to_u32(),
            priority.to_u32(),
            action,
        );
        env.events().publish(topics, data);
    }

    /// Emit a small batch-style event indicating bulk operations.
    ///
    /// The `action` parameter is included in the payload rather than as the final topic
    /// to make the topic schema consistent for batch analytics.
    pub fn emit_batch(env: &soroban_sdk::Env, category: EventCategory, action: Symbol, count: u32) {
        let topics = (
            symbol_short!("Remitwise"),
            category.to_u32(),
            EventPriority::Low.to_u32(),
            symbol_short!("batch"),
        );
        let data = (action, count);
        env.events().publish(topics, data);
    }
}

// Standardized TTL Constants (Ledger Counts)
pub const DAY_IN_LEDGERS: u32 = 17280; // ~5 seconds per ledger
pub const PERSISTENT_BUMP_AMOUNT: u32 = 60 * DAY_IN_LEDGERS; // 60 days
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = 15 * DAY_IN_LEDGERS; // 15 days

#[cfg(test)]
mod clamp_limit_tests {
    use super::*;

    // ── Zero input ────────────────────────────────────────────────────────────

    #[test]
    fn test_clamp_zero_returns_default() {
        assert_eq!(clamp_limit(0), DEFAULT_PAGE_LIMIT);
    }

    // ── Values within range ───────────────────────────────────────────────────

    #[test]
    fn test_clamp_one_returns_one() {
        assert_eq!(clamp_limit(1), 1);
    }

    #[test]
    fn test_clamp_midrange_returns_same() {
        let mid = DEFAULT_PAGE_LIMIT;
        assert_eq!(clamp_limit(mid), mid);
    }

    #[test]
    fn test_clamp_max_returns_max() {
        assert_eq!(clamp_limit(MAX_PAGE_LIMIT), MAX_PAGE_LIMIT);
    }

    // ── Values above max ──────────────────────────────────────────────────────

    #[test]
    fn test_clamp_above_max_returns_max() {
        assert_eq!(clamp_limit(MAX_PAGE_LIMIT + 1), MAX_PAGE_LIMIT);
    }

    #[test]
    fn test_clamp_large_value_returns_max() {
        assert_eq!(clamp_limit(u32::MAX), MAX_PAGE_LIMIT);
    }

    // ── Invariant: return always in [1, MAX_PAGE_LIMIT] ───────────────────────

    #[test]
    fn test_clamp_result_never_exceeds_max() {
        for v in [0u32, 1, 5, 19, 20, 21, 49, 50, 51, 100, u32::MAX] {
            let result = clamp_limit(v);
            assert!(
                result >= 1 && result <= MAX_PAGE_LIMIT,
                "clamp_limit({v}) = {result} is out of [1, {MAX_PAGE_LIMIT}]"
            );
        }
    }

    #[test]
    fn test_clamp_result_never_zero() {
        for v in [0u32, 1, MAX_PAGE_LIMIT, u32::MAX] {
            assert_ne!(clamp_limit(v), 0, "clamp_limit({v}) must never be 0");
        }
    }

    // ── Default vs max alignment ──────────────────────────────────────────────

    #[test]
    fn test_default_page_limit_within_max() {
        assert!(
            DEFAULT_PAGE_LIMIT <= MAX_PAGE_LIMIT,
            "DEFAULT_PAGE_LIMIT must be <= MAX_PAGE_LIMIT"
        );
    }

    #[test]
    fn test_default_page_limit_nonzero() {
        assert!(DEFAULT_PAGE_LIMIT > 0, "DEFAULT_PAGE_LIMIT must be > 0");
    }

    // ── Idempotency: clamping an already-clamped value is a no-op ─────────────

    #[test]
    fn test_clamp_idempotent() {
        for v in [0u32, 1, DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT, u32::MAX] {
            let once = clamp_limit(v);
            let twice = clamp_limit(once);
            assert_eq!(once, twice, "clamp_limit must be idempotent for input {v}");
        }
    }
}
