//! # Lifecycle Tests — Initialization & Events (Issue #757)
#![cfg(test)]
 
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, LedgerInfo},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, IntoVal, Symbol, TryIntoVal,
};
 
const BASE_TS: u64 = 1_700_000_000;
const FUTURE_DL: u64 = BASE_TS + 86_400;
const DEFAULT_AMOUNT: i128 = 10_000;
 
// ── helpers ───────────────────────────────────────────────────────────────────
 
type AllEvents = soroban_sdk::Vec<(Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val)>;
 
fn has_topic(env: &Env, all: &AllEvents, sym: Symbol) -> bool {
    for i in 0..all.len() {
        let (_, topics, _) = all.get(i).unwrap();
        if topics.len() > 0 {
            let result: Result<Symbol, _> = topics.get(0).unwrap().try_into_val(env);
            if let Ok(s) = result {
                if s == sym { return true; }
            }
        }
    }
    false
}
 
fn find_data(env: &Env, all: &AllEvents, sym: Symbol) -> Option<soroban_sdk::Val> {
    for i in 0..all.len() {
        let (_, topics, data) = all.get(i).unwrap();
        if topics.len() > 0 {
            let result: Result<Symbol, _> = topics.get(0).unwrap().try_into_val(env);
            if let Ok(s) = result {
                if s == sym { return Some(data); }
            }
        }
    }
    None
}
 
// ── harness ───────────────────────────────────────────────────────────────────
 
struct Ctx {
    env: Env,
    client: BountyEscrowContractClient<'static>,
    token_id: Address,
    admin: Address,
    depositor: Address,
    contributor: Address,
}
 
fn setup() -> Ctx {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo { timestamp: BASE_TS, ..Default::default() });
 
    let admin      = Address::generate(&env);
    let depositor  = Address::generate(&env);
    let contributor = Address::generate(&env);
 
    let token_ref = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let token_id  = token_ref.address();
    let sac: StellarAssetClient<'static> =
        unsafe { core::mem::transmute(StellarAssetClient::new(&env, &token_id)) };
    sac.mint(&depositor, &1_000_000);
 
    let cid = env.register_contract(None, BountyEscrowContract);
    let client: BountyEscrowContractClient<'static> =
        unsafe { core::mem::transmute(BountyEscrowContractClient::new(&env, &cid)) };
 
    Ctx { env, client, token_id, admin, depositor, contributor }
}
 
fn setup_init() -> Ctx {
    let ctx = setup();
    ctx.client.init(&ctx.admin, &ctx.token_id);
    ctx
}
 
fn lock(ctx: &Ctx, bounty_id: u64, amount: i128) {
    ctx.client.lock_funds(&ctx.depositor, &bounty_id, &amount, &FUTURE_DL);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// INIT — happy paths
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_init_happy_path() {
    let ctx = setup();
    assert!(ctx.client.try_init(&ctx.admin, &ctx.token_id).is_ok());
}
 
#[test]
fn test_init_emits_bounty_escrow_initialized() {
    let ctx = setup();
    ctx.client.init(&ctx.admin, &ctx.token_id);
    let all = ctx.env.events().all();
    assert!(has_topic(&ctx.env, &all, symbol_short!("init")),
        "BountyEscrowInitialized must be emitted");
}
 
#[test]
fn test_init_event_carries_version_v2() {
    let ctx = setup();
    ctx.client.init(&ctx.admin, &ctx.token_id);
    let all  = ctx.env.events().all();
    let data = find_data(&ctx.env, &all, symbol_short!("init")).expect("init event missing");
    let p: events::BountyEscrowInitialized = data.into_val(&ctx.env);
    assert_eq!(p.version, EVENT_VERSION_V2);
}
 
#[test]
fn test_init_event_fields_match_inputs() {
    let ctx = setup();
    ctx.client.init(&ctx.admin, &ctx.token_id);
    let all  = ctx.env.events().all();
    let data = find_data(&ctx.env, &all, symbol_short!("init")).expect("init event missing");
    let p: events::BountyEscrowInitialized = data.into_val(&ctx.env);
    assert_eq!(p.version,   EVENT_VERSION_V2);
    assert_eq!(p.admin,     ctx.admin);
    assert_eq!(p.token,     ctx.token_id);
    assert_eq!(p.timestamp, BASE_TS);
}
 
#[test]
fn test_balance_zero_after_init() {
    let ctx = setup_init();
    assert_eq!(ctx.client.get_balance(), 0);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// INIT — error paths
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_init_already_initialized_error() {
    let ctx = setup_init();
    let r = ctx.client.try_init(&ctx.admin, &ctx.token_id);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::AlreadyInitialized);
}
 
#[test]
fn test_init_admin_equals_token_rejected() {
    let ctx = setup();
    let dup = ctx.token_id.clone();
    let r = ctx.client.try_init(&dup, &ctx.token_id);
    assert!(r.is_err(), "admin == token must be rejected");
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// INIT WITH NETWORK
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_init_with_network_happy_path() {
    let ctx = setup();
    let chain = soroban_sdk::String::from_str(&ctx.env, "stellar");
    let net   = soroban_sdk::String::from_str(&ctx.env, "mainnet");
    assert!(ctx.client.try_init_with_network(&ctx.admin, &ctx.token_id, &chain, &net).is_ok());
    assert!(ctx.client.get_chain_id().is_some());
    assert!(ctx.client.get_network_id().is_some());
}
 
#[test]
fn test_init_with_network_replay_rejected() {
    let ctx = setup();
    let chain = soroban_sdk::String::from_str(&ctx.env, "stellar");
    let net   = soroban_sdk::String::from_str(&ctx.env, "testnet");
    ctx.client.init_with_network(&ctx.admin, &ctx.token_id, &chain, &net);
    let r = ctx.client.try_init_with_network(&ctx.admin, &ctx.token_id, &chain, &net);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::AlreadyInitialized);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// LOCK FUNDS
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_lock_funds_after_init() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    let info = ctx.client.get_escrow_info(&1u64);
    assert_eq!(info.status, EscrowStatus::Locked);
    assert_eq!(info.amount, DEFAULT_AMOUNT);
}
 
#[test]
fn test_lock_funds_emits_funds_locked() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    let all = ctx.env.events().all();
    assert!(has_topic(&ctx.env, &all, symbol_short!("f_lock")), "FundsLocked must be emitted");
}
 
#[test]
fn test_lock_funds_event_fields() {
    let ctx = setup_init();
    lock(&ctx, 99, DEFAULT_AMOUNT);
    let all  = ctx.env.events().all();
    let data = find_data(&ctx.env, &all, symbol_short!("f_lock")).expect("f_lock missing");
    let p: events::FundsLocked = data.into_val(&ctx.env);
    assert_eq!(p.version,   EVENT_VERSION_V2);
    assert_eq!(p.bounty_id, 99u64);
    assert_eq!(p.amount,    DEFAULT_AMOUNT);
    assert_eq!(p.depositor, ctx.depositor);
    assert_eq!(p.deadline,  FUTURE_DL);
}
 
#[test]
fn test_get_balance_reflects_locked_funds() {
    let ctx = setup_init();
    assert_eq!(ctx.client.get_balance(), 0);
    lock(&ctx, 1, DEFAULT_AMOUNT);
    assert_eq!(ctx.client.get_balance(), DEFAULT_AMOUNT);
    // Advance time to bypass cooldown period (default 60 seconds)
    ctx.env.ledger().set(LedgerInfo { timestamp: BASE_TS + 61, ..Default::default() });
    lock(&ctx, 2, 5_000);
    assert_eq!(ctx.client.get_balance(), DEFAULT_AMOUNT + 5_000);
}
 
#[test]
fn test_lock_funds_before_init_fails() {
    let ctx = setup();
    let r = ctx.client.try_lock_funds(&ctx.depositor, &1u64, &DEFAULT_AMOUNT, &FUTURE_DL);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::NotInitialized);
}
 
#[test]
fn test_lock_funds_zero_amount_fails() {
    let ctx = setup_init();
    let r = ctx.client.try_lock_funds(&ctx.depositor, &1u64, &0i128, &FUTURE_DL);
    assert!(r.is_err(), "zero amount must be rejected");
}
 
#[test]
fn test_lock_funds_duplicate_bounty_fails() {
    let ctx = setup_init();
    lock(&ctx, 7, DEFAULT_AMOUNT);
    // Advance time to bypass cooldown period (default 60 seconds)
    ctx.env.ledger().set(LedgerInfo { timestamp: BASE_TS + 61, ..Default::default() });
    let r = ctx.client.try_lock_funds(&ctx.depositor, &7u64, &DEFAULT_AMOUNT, &FUTURE_DL);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::BountyExists);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// RELEASE FUNDS
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_release_funds_happy_path() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.release_funds(&1u64, &ctx.contributor);
    let info = ctx.client.get_escrow_info(&1u64);
    assert_eq!(info.status, EscrowStatus::Released);
    assert_eq!(info.remaining_amount, 0);
}
 
#[test]
fn test_release_funds_emits_event() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.release_funds(&1u64, &ctx.contributor);
    let all = ctx.env.events().all();
    assert!(has_topic(&ctx.env, &all, symbol_short!("f_rel")), "FundsReleased must be emitted");
}
 
#[test]
fn test_release_funds_event_fields() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.release_funds(&1u64, &ctx.contributor);
    let all  = ctx.env.events().all();
    let data = find_data(&ctx.env, &all, symbol_short!("f_rel")).expect("f_rel missing");
    let p: events::FundsReleased = data.into_val(&ctx.env);
    assert_eq!(p.version,   EVENT_VERSION_V2);
    assert_eq!(p.bounty_id, 1u64);
    assert_eq!(p.amount,    DEFAULT_AMOUNT);
    assert_eq!(p.recipient, ctx.contributor);
}
 
#[test]
fn test_release_funds_bounty_not_found() {
    let ctx = setup_init();
    let r = ctx.client.try_release_funds(&99u64, &ctx.contributor);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::BountyNotFound);
}
 
#[test]
fn test_release_funds_double_release_fails() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.release_funds(&1u64, &ctx.contributor);
    let r = ctx.client.try_release_funds(&1u64, &ctx.contributor);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::FundsNotLocked);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// REFUND
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_refund_after_deadline_happy_path() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.env.ledger().set(LedgerInfo { timestamp: FUTURE_DL + 1, ..Default::default() });
    ctx.client.refund(&1u64);
    let info = ctx.client.get_escrow_info(&1u64);
    assert_eq!(info.status, EscrowStatus::Refunded);
    assert_eq!(info.remaining_amount, 0);
}
 
#[test]
fn test_refund_emits_event() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.env.ledger().set(LedgerInfo { timestamp: FUTURE_DL + 1, ..Default::default() });
    ctx.client.refund(&1u64);
    let all = ctx.env.events().all();
    assert!(has_topic(&ctx.env, &all, symbol_short!("f_ref")), "FundsRefunded must be emitted");
}
 
#[test]
fn test_refund_event_fields() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.env.ledger().set(LedgerInfo { timestamp: FUTURE_DL + 1, ..Default::default() });
    ctx.client.refund(&1u64);
    let all  = ctx.env.events().all();
    let data = find_data(&ctx.env, &all, symbol_short!("f_ref")).expect("f_ref missing");
    let p: events::FundsRefunded = data.into_val(&ctx.env);
    assert_eq!(p.version,   EVENT_VERSION_V2);
    assert_eq!(p.bounty_id, 1u64);
    assert_eq!(p.amount,    DEFAULT_AMOUNT);
    assert_eq!(p.refund_to, ctx.depositor);
}
 
#[test]
fn test_refund_before_deadline_no_approval_fails() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    let r = ctx.client.try_refund(&1u64);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::DeadlineNotPassed);
}
 
#[test]
fn test_refund_already_released_fails() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.release_funds(&1u64, &ctx.contributor);
    ctx.env.ledger().set(LedgerInfo { timestamp: FUTURE_DL + 1, ..Default::default() });
    let r = ctx.client.try_refund(&1u64);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::FundsNotLocked);
}
 
#[test]
fn test_early_refund_with_admin_approval() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.approve_refund(&1u64, &DEFAULT_AMOUNT, &ctx.depositor, &RefundMode::Full);
    ctx.client.refund(&1u64);
    assert_eq!(ctx.client.get_escrow_info(&1u64).status, EscrowStatus::Refunded);
}
 
#[test]
fn test_partial_refund_flow() {
    let ctx  = setup_init();
    let partial = 3_000i128;
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.approve_refund(&1u64, &partial, &ctx.depositor, &RefundMode::Partial);
    ctx.client.refund(&1u64);
    let info = ctx.client.get_escrow_info(&1u64);
    assert_eq!(info.status,           EscrowStatus::PartiallyRefunded);
    assert_eq!(info.remaining_amount, DEFAULT_AMOUNT - partial);
    assert_eq!(ctx.client.get_refund_history(&1u64).get(0).unwrap().amount, partial);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// PAUSE / DEPRECATION / MAINTENANCE
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_lock_paused_blocks_lock_funds() {
    let ctx = setup_init();
    ctx.client.set_paused(&Some(true), &None, &None, &None);
    let r = ctx.client.try_lock_funds(&ctx.depositor, &1u64, &DEFAULT_AMOUNT, &FUTURE_DL);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::FundsPaused);
}
 
#[test]
fn test_release_paused_blocks_release() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.set_paused(&None, &Some(true), &None, &None);
    let r = ctx.client.try_release_funds(&1u64, &ctx.contributor);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::FundsPaused);
}
 
#[test]
fn test_deprecated_blocks_lock_funds() {
    let ctx = setup_init();
    ctx.client.set_deprecated(&true, &None);
    let r = ctx.client.try_lock_funds(&ctx.depositor, &1u64, &DEFAULT_AMOUNT, &FUTURE_DL);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::ContractDeprecated);
}
 
#[test]
fn test_maintenance_mode_blocks_lock() {
    let ctx = setup_init();
    ctx.client.set_maintenance_mode(&true);
    let r = ctx.client.try_lock_funds(&ctx.depositor, &1u64, &DEFAULT_AMOUNT, &FUTURE_DL);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::FundsPaused);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// EMERGENCY WITHDRAW
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_emergency_withdraw_requires_paused() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    let r = ctx.client.try_emergency_withdraw(&ctx.admin);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::NotPaused);
}
 
#[test]
fn test_emergency_withdraw_happy_path() {
    let ctx    = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.set_paused(&Some(true), &None, &None, &None);
    let target = Address::generate(&ctx.env);
    let tc     = TokenClient::new(&ctx.env, &ctx.token_id);
    let before = tc.balance(&target);
    ctx.client.emergency_withdraw(&target);
    assert_eq!(tc.balance(&target) - before, DEFAULT_AMOUNT);
    assert_eq!(ctx.client.get_balance(), 0);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// OPERATIONAL STATE EVENTS
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_deprecation_emits_event() {
    let ctx  = setup_init();
    ctx.client.set_deprecated(&true, &None);
    let all  = ctx.env.events().all();
    let data = find_data(&ctx.env, &all, symbol_short!("deprec")).expect("deprec event missing");
    let p: events::DeprecationStateChanged = data.into_val(&ctx.env);
    assert!(p.deprecated);
    assert_eq!(p.admin, ctx.admin);
}
 
#[test]
fn test_maintenance_mode_emits_event() {
    let ctx  = setup_init();
    ctx.client.set_maintenance_mode(&true);
    let all  = ctx.env.events().all();
    let data = find_data(&ctx.env, &all, symbol_short!("maint")).expect("maint event missing");
    let p: events::MaintenanceModeChanged = data.into_val(&ctx.env);
    assert!(p.enabled);
    assert_eq!(p.admin, ctx.admin);
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// ALL VERSIONED EVENTS CARRY EVENT_VERSION_V2
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_all_lifecycle_events_carry_v2_version() {
    let ctx = setup_init();
    lock(&ctx, 1, DEFAULT_AMOUNT);
    ctx.client.release_funds(&1u64, &ctx.contributor);
 
    let all = ctx.env.events().all();
    for i in 0..all.len() {
        let (_, topics, data) = all.get(i).unwrap();
        if topics.len() == 0 { continue; }
        let result: Result<Symbol, _> = topics.get(0).unwrap().try_into_val(&ctx.env);
        let Ok(sym) = result else { continue };{ continue };
 
        if sym == symbol_short!("init") {
            let p: events::BountyEscrowInitialized = data.into_val(&ctx.env);
            assert_eq!(p.version, EVENT_VERSION_V2, "init: wrong version");
        } else if sym == symbol_short!("f_lock") {
            let p: events::FundsLocked = data.into_val(&ctx.env);
            assert_eq!(p.version, EVENT_VERSION_V2, "f_lock: wrong version");
        } else if sym == symbol_short!("f_rel") {
            let p: events::FundsReleased = data.into_val(&ctx.env);
            assert_eq!(p.version, EVENT_VERSION_V2, "f_rel: wrong version");
        } else if sym == symbol_short!("f_ref") {
            let p: events::FundsRefunded = data.into_val(&ctx.env);
            assert_eq!(p.version, EVENT_VERSION_V2, "f_ref: wrong version");
        }
    }
}
 
// ═══════════════════════════════════════════════════════════════════════════════
// NOT-FOUND GUARDS
// ═══════════════════════════════════════════════════════════════════════════════
 
#[test]
fn test_get_escrow_info_not_found() {
    let ctx = setup_init();
    let r = ctx.client.try_get_escrow_info(&9999u64);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::BountyNotFound);
}
 
#[test]
fn test_refund_bounty_not_found() {
    let ctx = setup_init();
    let r = ctx.client.try_refund(&9999u64);
    assert!(r.is_err());
    assert_eq!(r.unwrap_err().unwrap(), Error::BountyNotFound);
}