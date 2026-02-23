#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
    token,
};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditStatus {
    Active = 0,
    Suspended = 1,
    Defaulted = 2,
    Closed = 3,
}

#[contracttype]
pub struct CreditLineData {
    pub borrower: Address,
    pub credit_limit: i128,
    pub utilized_amount: i128,
    pub interest_rate_bps: u32,
    pub risk_score: u32,
    pub status: CreditStatus,
}

/// Event emitted when a credit line lifecycle event occurs
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditLineEvent {
    pub event_type: Symbol,
    pub borrower: Address,
    pub status: CreditStatus,
    pub credit_limit: i128,
    pub interest_rate_bps: u32,
    pub risk_score: u32,
}

/// Storage keys
const ADMIN_KEY: &str = "admin";
const TOKEN_KEY: &str = "token";

#[contract]
pub struct Credit;

#[contractimpl]
impl Credit {
    /// Initialize the contract with admin and reserve token address.
    pub fn init(env: Env, admin: Address, token: Address) {
        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &admin);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, TOKEN_KEY), &token);
    }

    /// Open a new credit line for a borrower (called by backend/risk engine).
    /// Emits a CreditLineOpened event.
    pub fn open_credit_line(
        env: Env,
        borrower: Address,
        credit_limit: i128,
        interest_rate_bps: u32,
        risk_score: u32,
    ) {
        let credit_line = CreditLineData {
            borrower: borrower.clone(),
            credit_limit,
            utilized_amount: 0,
            interest_rate_bps,
            risk_score,
            status: CreditStatus::Active,
        };

        env.storage().persistent().set(&borrower, &credit_line);

        env.events().publish(
            (symbol_short!("credit"), symbol_short!("opened")),
            CreditLineEvent {
                event_type: symbol_short!("opened"),
                borrower: borrower.clone(),
                status: CreditStatus::Active,
                credit_limit,
                interest_rate_bps,
                risk_score,
            },
        );
    }

    /// Draw from credit line: verifies limit, updates utilized_amount,
    /// and transfers the protocol token from the contract reserve to the borrower.
    ///
    /// # Panics
    /// - `"Credit line not found"` – borrower has no open credit line
    /// - `"Credit line not active"` – line is suspended, defaulted, or closed
    /// - `"Exceeds credit limit"` – draw would push utilized_amount past credit_limit
    /// - `"Invalid amount"` – amount is zero or negative
    pub fn draw_credit(env: Env, borrower: Address, amount: i128) {
        borrower.require_auth();

        assert!(amount > 0, "Invalid amount");

        let mut credit_line: CreditLineData = env
            .storage()
            .persistent()
            .get(&borrower)
            .expect("Credit line not found");

        assert!(
            credit_line.status == CreditStatus::Active,
            "Credit line not active"
        );

        let new_utilized = credit_line
            .utilized_amount
            .checked_add(amount)
            .expect("Overflow");

        assert!(new_utilized <= credit_line.credit_limit, "Exceeds credit limit");

        // Update state before external call (checks-effects-interactions)
        credit_line.utilized_amount = new_utilized;
        env.storage().persistent().set(&borrower, &credit_line);

        // Transfer token from contract reserve to borrower
        let token_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, TOKEN_KEY))
            .expect("Token not configured");

        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&env.current_contract_address(), &borrower, &amount);

        // Emit draw event
        env.events().publish(
            (symbol_short!("credit"), symbol_short!("draw")),
            (borrower, amount, new_utilized),
        );
    }

    /// Repay credit (borrower).
    pub fn repay_credit(_env: Env, _borrower: Address, _amount: i128) {
        // TODO: accept token, reduce utilized_amount, accrue interest
    }

    /// Update risk parameters (admin/risk engine).
    pub fn update_risk_parameters(
        _env: Env,
        _borrower: Address,
        _credit_limit: i128,
        _interest_rate_bps: u32,
        _risk_score: u32,
    ) {
        // TODO: update stored CreditLineData
    }

    /// Suspend a credit line (admin).
    pub fn suspend_credit_line(env: Env, borrower: Address) {
        let mut credit_line: CreditLineData = env
            .storage()
            .persistent()
            .get(&borrower)
            .expect("Credit line not found");

        credit_line.status = CreditStatus::Suspended;
        env.storage().persistent().set(&borrower, &credit_line);

        env.events().publish(
            (symbol_short!("credit"), symbol_short!("suspend")),
            CreditLineEvent {
                event_type: symbol_short!("suspend"),
                borrower: borrower.clone(),
                status: CreditStatus::Suspended,
                credit_limit: credit_line.credit_limit,
                interest_rate_bps: credit_line.interest_rate_bps,
                risk_score: credit_line.risk_score,
            },
        );
    }

    /// Close a credit line (admin or borrower when utilized is 0).
    pub fn close_credit_line(env: Env, borrower: Address) {
        let mut credit_line: CreditLineData = env
            .storage()
            .persistent()
            .get(&borrower)
            .expect("Credit line not found");

        credit_line.status = CreditStatus::Closed;
        env.storage().persistent().set(&borrower, &credit_line);

        env.events().publish(
            (symbol_short!("credit"), symbol_short!("closed")),
            CreditLineEvent {
                event_type: symbol_short!("closed"),
                borrower: borrower.clone(),
                status: CreditStatus::Closed,
                credit_limit: credit_line.credit_limit,
                interest_rate_bps: credit_line.interest_rate_bps,
                risk_score: credit_line.risk_score,
            },
        );
    }

    /// Mark a credit line as defaulted (admin).
    pub fn default_credit_line(env: Env, borrower: Address) {
        let mut credit_line: CreditLineData = env
            .storage()
            .persistent()
            .get(&borrower)
            .expect("Credit line not found");

        credit_line.status = CreditStatus::Defaulted;
        env.storage().persistent().set(&borrower, &credit_line);

        env.events().publish(
            (symbol_short!("credit"), symbol_short!("default")),
            CreditLineEvent {
                event_type: symbol_short!("default"),
                borrower: borrower.clone(),
                status: CreditStatus::Defaulted,
                credit_limit: credit_line.credit_limit,
                interest_rate_bps: credit_line.interest_rate_bps,
                risk_score: credit_line.risk_score,
            },
        );
    }

    /// Get credit line data for a borrower (view function).
    pub fn get_credit_line(env: Env, borrower: Address) -> Option<CreditLineData> {
        env.storage().persistent().get(&borrower)
    }
}

// Tests
#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::Address as _,
        token,
    };

    // helpers 

    /// Deploy a Soroban test token, mint `reserve_amount` to the credit
    /// contract's address, and return (token_id, token_admin_client).
    fn setup_token<'a>(
        env: &'a Env,
        contract_id: &'a Address,
        reserve_amount: i128,
    ) -> (Address, token::StellarAssetClient<'a>) {
        let token_admin = Address::generate(env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_address = token_id.address();
        let sac = token::StellarAssetClient::new(env, &token_address);

        // Only mint when there is an actual reserve — minting 0 requires auth
        // and causes unrelated auth failures in tests that only need the token registered.
        if reserve_amount > 0 {
            sac.mint(contract_id, &reserve_amount);
        }

        (token_address, sac)
    }

    /// Register the Credit contract, init it, open a credit line, and fund
    /// the reserve.  Returns `(client, token_address)`.
    fn setup_contract_with_credit_line<'a>(
        env: &'a Env,
        borrower: &'a Address,
        credit_limit: i128,
        reserve_amount: i128,
    ) -> (CreditClient<'a>, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register(Credit, ());
        let (token_address, _sac) = setup_token(env, &contract_id, reserve_amount);
        let client = CreditClient::new(env, &contract_id);

        client.init(&admin, &token_address);
        client.open_credit_line(borrower, &credit_limit, &300_u32, &70_u32);

        (client, token_address)
    }

    // draw_credit: token transfer

    /// Core requirement: the exact requested amount arrives in borrower's wallet.
    #[test]
    fn test_draw_transfers_correct_amount_to_borrower() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, token_address) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        let token_client = token::Client::new(&env, &token_address);
        let before = token_client.balance(&borrower);

        client.draw_credit(&borrower, &500);

        let after = token_client.balance(&borrower);
        assert_eq!(after - before, 500, "borrower should receive exactly 500");
    }

    /// Reserve balance must decrease by the drawn amount.
    #[test]
    fn test_draw_reduces_contract_reserve() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let contract_id = env.register(Credit, ());
        let (token_address, _sac) = setup_token(&env, &contract_id, 1_000);
        let admin = Address::generate(&env);
        let client = CreditClient::new(&env, &contract_id);
        client.init(&admin, &token_address);
        client.open_credit_line(&borrower, &1_000, &300_u32, &70_u32);

        let token_client = token::Client::new(&env, &token_address);
        let reserve_before = token_client.balance(&contract_id);

        client.draw_credit(&borrower, &300);

        let reserve_after = token_client.balance(&contract_id);
        assert_eq!(
            reserve_before - reserve_after,
            300,
            "contract reserve should decrease by drawn amount"
        );
    }

    /// utilized_amount in storage must be updated after a draw.
    #[test]
    fn test_draw_updates_utilized_amount() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.draw_credit(&borrower, &400);

        let line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(line.utilized_amount, 400);
    }

    /// Two sequential draws must both transfer correctly and accumulate
    /// utilized_amount.
    #[test]
    fn test_draw_accumulates_across_multiple_draws() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, token_address) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.draw_credit(&borrower, &200);
        client.draw_credit(&borrower, &300);

        let token_client = token::Client::new(&env, &token_address);
        assert_eq!(token_client.balance(&borrower), 500);

        let line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(line.utilized_amount, 500);
    }

    /// Drawing the full credit limit in one call must succeed.
    #[test]
    fn test_draw_exact_credit_limit() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, token_address) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.draw_credit(&borrower, &1_000);

        let token_client = token::Client::new(&env, &token_address);
        assert_eq!(token_client.balance(&borrower), 1_000);

        let line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(line.utilized_amount, 1_000);
    }

    /// draw_credit must require the borrower's authorization signature.
    /// We verify this by using mock_all_auths (which records all auth calls)
    /// and then asserting the borrower's address appears in the recorded auths.
    #[test]
    fn test_draw_requires_borrower_auth() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.draw_credit(&borrower, &100);

        // Confirm that borrower auth was required during the draw call
        let auths = env.auths();
        let borrower_auth_found = auths.iter().any(|(addr, _)| *addr == borrower);
        assert!(borrower_auth_found, "draw_credit must require borrower authorization");
    }

    // draw_credit: guard / negative cases 

    /// Drawing more than the available credit limit must panic.
    #[test]
    #[should_panic(expected = "Exceeds credit limit")]
    fn test_draw_exceeds_credit_limit() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 500, 1_000);

        client.draw_credit(&borrower, &600);
    }

    /// A second draw that would push past the limit must also be rejected.
    #[test]
    #[should_panic(expected = "Exceeds credit limit")]
    fn test_draw_cumulative_exceeds_limit() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 500, 1_000);

        client.draw_credit(&borrower, &400);
        client.draw_credit(&borrower, &200); // 400 + 200 > 500 → panic
    }

    /// Drawing on a suspended credit line must be rejected.
    #[test]
    #[should_panic(expected = "Credit line not active")]
    fn test_draw_on_suspended_line_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.suspend_credit_line(&borrower);
        client.draw_credit(&borrower, &100);
    }

    /// Drawing on a closed credit line must be rejected.
    #[test]
    #[should_panic(expected = "Credit line not active")]
    fn test_draw_on_closed_line_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.close_credit_line(&borrower);
        client.draw_credit(&borrower, &100);
    }

    /// Drawing on a defaulted credit line must be rejected.
    #[test]
    #[should_panic(expected = "Credit line not active")]
    fn test_draw_on_defaulted_line_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.default_credit_line(&borrower);
        client.draw_credit(&borrower, &100);
    }

    /// Drawing with amount = 0 must be rejected.
    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_draw_zero_amount_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.draw_credit(&borrower, &0);
    }

    /// Drawing with a negative amount must be rejected.
    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_draw_negative_amount_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _token) =
            setup_contract_with_credit_line(&env, &borrower, 1_000, 1_000);

        client.draw_credit(&borrower, &-50);
    }

    /// Drawing for a borrower who has no credit line must panic.
    #[test]
    #[should_panic(expected = "Credit line not found")]
    fn test_draw_no_credit_line_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let stranger = Address::generate(&env);
        let admin = Address::generate(&env);
        let contract_id = env.register(Credit, ());
        let (token_address, _sac) = setup_token(&env, &contract_id, 1_000);
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin, &token_address);
        client.draw_credit(&stranger, &100);
    }

    // existing lifecycle tests (updated for new init signature) 

    #[test]
    fn test_init_and_open_credit_line() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);
        let contract_id = env.register(Credit, ());
        let (token_address, _) = setup_token(&env, &contract_id, 0);
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin, &token_address);
        client.open_credit_line(&borrower, &1000_i128, &300_u32, &70_u32);

        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.borrower, borrower);
        assert_eq!(credit_line.credit_limit, 1000);
        assert_eq!(credit_line.utilized_amount, 0);
        assert_eq!(credit_line.interest_rate_bps, 300);
        assert_eq!(credit_line.risk_score, 70);
        assert_eq!(credit_line.status, CreditStatus::Active);
    }

    #[test]
    fn test_suspend_credit_line() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _) = setup_contract_with_credit_line(&env, &borrower, 1_000, 0);

        client.suspend_credit_line(&borrower);
        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Suspended
        );
    }

    #[test]
    fn test_close_credit_line() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _) = setup_contract_with_credit_line(&env, &borrower, 1_000, 0);

        client.close_credit_line(&borrower);
        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Closed
        );
    }

    #[test]
    fn test_default_credit_line() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _) = setup_contract_with_credit_line(&env, &borrower, 1_000, 0);

        client.default_credit_line(&borrower);
        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Defaulted
        );
    }

    #[test]
    fn test_full_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        let borrower = Address::generate(&env);
        let (client, _) = setup_contract_with_credit_line(&env, &borrower, 5_000, 5_000);

        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Active
        );
        client.suspend_credit_line(&borrower);
        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Suspended
        );
        client.close_credit_line(&borrower);
        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Closed
        );
    }

    #[test]
    fn test_multiple_borrowers() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);
        let contract_id = env.register(Credit, ());
        let (token_address, sac) = setup_token(&env, &contract_id, 3_000);
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin, &token_address);
        client.open_credit_line(&b1, &1_000, &300_u32, &70_u32);
        client.open_credit_line(&b2, &2_000, &400_u32, &80_u32);

        // Both can draw independently
        client.draw_credit(&b1, &500);
        client.draw_credit(&b2, &1_000);

        let token_client = token::Client::new(&env, &token_address);
        assert_eq!(token_client.balance(&b1), 500);
        assert_eq!(token_client.balance(&b2), 1_000);

        assert_eq!(
            client.get_credit_line(&b1).unwrap().utilized_amount,
            500
        );
        assert_eq!(
            client.get_credit_line(&b2).unwrap().utilized_amount,
            1_000
        );

        // Silence unused variable warning
        let _ = sac;
    }

    #[test]
    #[should_panic(expected = "Credit line not found")]
    fn test_suspend_nonexistent_credit_line() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);
        let contract_id = env.register(Credit, ());
        let (token_address, _) = setup_token(&env, &contract_id, 0);
        let client = CreditClient::new(&env, &contract_id);
        client.init(&admin, &token_address);
        client.suspend_credit_line(&borrower);
    }

    #[test]
    #[should_panic(expected = "Credit line not found")]
    fn test_close_nonexistent_credit_line() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);
        let contract_id = env.register(Credit, ());
        let (token_address, _) = setup_token(&env, &contract_id, 0);
        let client = CreditClient::new(&env, &contract_id);
        client.init(&admin, &token_address);
        client.close_credit_line(&borrower);
    }

    #[test]
    #[should_panic(expected = "Credit line not found")]
    fn test_default_nonexistent_credit_line() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);
        let contract_id = env.register(Credit, ());
        let (token_address, _) = setup_token(&env, &contract_id, 0);
        let client = CreditClient::new(&env, &contract_id);
        client.init(&admin, &token_address);
        client.default_credit_line(&borrower);
    }
}