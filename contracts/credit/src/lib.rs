#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditStatus {
    Active = 0,
    Suspended = 1,
    Defaulted = 2,
    Closed = 3,
}

#[contracttype]
#[derive(Debug, Clone, PartialEq)]
pub enum CreditError {
    CreditLineNotFound = 1,
    InvalidCreditStatus = 2,
    InvalidAmount = 3,
    InsufficientUtilization = 4,
    Unauthorized = 5,
}

impl Into<soroban_sdk::Error> for CreditError {
    fn into(self) -> soroban_sdk::Error {
        soroban_sdk::Error::from_contract_error(self as u32)
    }
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

#[contract]
pub struct Credit;

#[contractimpl]
impl Credit {
    /// Initialize the contract (admin).
    pub fn init(env: Env, admin: Address) -> () {
        env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);
        ()
    }

    /// Open a new credit line for a borrower (called by backend/risk engine).
    /// Emits a CreditLineOpened event.
    pub fn open_credit_line(
        env: Env,
        borrower: Address,
        credit_limit: i128,
        interest_rate_bps: u32,
        risk_score: u32,
    ) -> () {
        let credit_line = CreditLineData {
            borrower: borrower.clone(),
            credit_limit,
            utilized_amount: 0,
            interest_rate_bps,
            risk_score,
            status: CreditStatus::Active,
        };

        env.storage()
            .persistent()
            .set(&borrower, &credit_line);

        // Emit CreditLineOpened event
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
        ()
    }

    /// Draw from credit line (borrower).
    pub fn draw_credit(env: Env, borrower: Address, amount: i128) -> () {
        if amount <= 0 {
            panic_with_error!(&env, CreditError::InvalidAmount);
        }

        let credit_key = (Symbol::new(&env, "CREDIT_LINE"), borrower.clone());
        let mut credit_data: CreditLineData = env.storage().persistent().get(&credit_key)
            .unwrap_or_else(|| panic_with_error!(&env, CreditError::CreditLineNotFound));

        if credit_data.status != CreditStatus::Active {
            panic_with_error!(&env, CreditError::InvalidCreditStatus);
        }

        let available_credit = credit_data.credit_limit.checked_sub(credit_data.utilized_amount)
            .expect("Credit limit should be >= utilized amount");
        
        if amount > available_credit {
            panic_with_error!(&env, CreditError::InsufficientUtilization);
        }

        credit_data.utilized_amount = credit_data.utilized_amount.checked_add(amount)
            .expect("Utilized amount should not overflow credit limit");

        env.storage().persistent().set(&credit_key, &credit_data);

        // Emit draw event
        env.events().publish(
            (Symbol::new(&env, "draw"), borrower.clone()),
            (amount, credit_data.utilized_amount)
        );
    }

    /// Repay credit (borrower).
    /// 
    /// Repays the specified amount from the borrower's credit line.
    /// The amount is applied to reduce the utilized_amount, with any excess
    /// amount ignored (no refund for overpayment).
    /// 
    /// # Arguments
    /// * `borrower` - The address of the borrower making the repayment
    /// * `amount` - The repayment amount (must be > 0)
    /// 
    /// # Errors
    /// * `CreditLineNotFound` - If no credit line exists for the borrower
    /// * `InvalidCreditStatus` - If credit line is not Active or Suspended
    /// * `InvalidAmount` - If amount <= 0
    /// 
    /// # Events
    /// Emits a repayment event with borrower address and amount applied
    pub fn repay_credit(env: Env, borrower: Address, amount: i128) -> () {
        // Validate input
        if amount <= 0 {
            panic_with_error!(&env, CreditError::InvalidAmount);
        }

        // Get credit line data
        let credit_key = (Symbol::new(&env, "CREDIT_LINE"), borrower.clone());
        let mut credit_data: CreditLineData = env.storage().persistent().get(&credit_key)
            .unwrap_or_else(|| panic_with_error!(&env, CreditError::CreditLineNotFound));

        // Validate credit status
        if credit_data.status != CreditStatus::Active && credit_data.status != CreditStatus::Suspended {
            panic_with_error!(&env, CreditError::InvalidCreditStatus);
        }

        // Calculate amount to apply (capped at current utilization)
        let amount_to_apply = if amount > credit_data.utilized_amount {
            credit_data.utilized_amount
        } else {
            amount
        };

        // Update utilized amount
        credit_data.utilized_amount = credit_data.utilized_amount.checked_sub(amount_to_apply)
            .expect("Underflow should not occur with proper validation");

        // Store updated credit line data
        env.storage().persistent().set(&credit_key, &credit_data);

        // Emit repayment event
        env.events().publish(
            (Symbol::new(&env, "repayment"), borrower.clone()),
            (amount_to_apply, credit_data.utilized_amount)
        );

        ()
    }

    /// Update risk parameters (admin/risk engine).
    pub fn update_risk_parameters(
        _env: Env,
        _borrower: Address,
        _credit_limit: i128,
        _interest_rate_bps: u32,
        _risk_score: u32,
    ) -> () {
        // TODO: update stored CreditLineData
        ()
    }

    /// Suspend a credit line (admin).
    /// Emits a CreditLineSuspended event.
    pub fn suspend_credit_line(env: Env, borrower: Address) -> () {
        let mut credit_line: CreditLineData = env
            .storage()
            .persistent()
            .get(&borrower)
            .expect("Credit line not found");

        credit_line.status = CreditStatus::Suspended;
        env.storage()
            .persistent()
            .set(&borrower, &credit_line);

        // Emit CreditLineSuspended event
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
        ()
    }

    /// Close a credit line (admin or borrower when utilized is 0).
    /// Emits a CreditLineClosed event.
    pub fn close_credit_line(env: Env, borrower: Address) -> () {
        let mut credit_line: CreditLineData = env
            .storage()
            .persistent()
            .get(&borrower)
            .expect("Credit line not found");

        credit_line.status = CreditStatus::Closed;
        env.storage()
            .persistent()
            .set(&borrower, &credit_line);

        // Emit CreditLineClosed event
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
        ()
    }

    /// Mark a credit line as defaulted (admin).
    /// Emits a CreditLineDefaulted event.
    pub fn default_credit_line(env: Env, borrower: Address) -> () {
        let mut credit_line: CreditLineData = env
            .storage()
            .persistent()
            .get(&borrower)
            .expect("Credit line not found");

        credit_line.status = CreditStatus::Defaulted;
        env.storage()
            .persistent()
            .set(&borrower, &credit_line);

        // Emit CreditLineDefaulted event
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
        ()
    }

    /// Get credit line data for a borrower (view function).
    pub fn get_credit_line(env: Env, borrower: Address) -> Option<CreditLineData> {
        env.storage().persistent().get(&borrower)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Symbol;

    fn call_contract<F>(env: &Env, contract_id: &Address, f: F) 
    where F: FnOnce() {
        env.as_contract(contract_id, f);
    }

    fn setup_test(env: &Env) -> (Address, Address, Address) {
        let admin = Address::generate(env);
        let borrower = Address::generate(env);
        let contract_id = env.register(Credit, ());
        
        env.as_contract(&contract_id, || {
            Credit::init(env.clone(), admin.clone());
            Credit::open_credit_line(env.clone(), borrower.clone(), 1000_i128, 300_u32, 70_u32);
        });
        
        (admin, borrower, contract_id)
    }

    fn get_credit_data(env: &Env, contract_id: &Address, borrower: &Address) -> CreditLineData {
        let credit_key = (Symbol::new(env, "CREDIT_LINE"), borrower.clone());
        env.as_contract(contract_id, || {
            env.storage().persistent().get(&credit_key).unwrap()
        })
    }

    #[test]
    fn test_init_and_open_credit_line() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.open_credit_line(&borrower, &1000_i128, &300_u32, &70_u32);

        // Verify credit line was created
        let credit_line = client.get_credit_line(&borrower);
        assert!(credit_line.is_some());
        let credit_line = credit_line.unwrap();
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
        
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.open_credit_line(&borrower, &1000_i128, &300_u32, &70_u32);
        client.suspend_credit_line(&borrower);

        // Verify status changed to Suspended
        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.status, CreditStatus::Suspended);
    }

    #[test]
    fn test_close_credit_line() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.open_credit_line(&borrower, &1000_i128, &300_u32, &70_u32);
        client.close_credit_line(&borrower);

        // Verify status changed to Closed
        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.status, CreditStatus::Closed);
    }

    #[test]
    fn test_default_credit_line() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

    #[test]
    fn test_draw_credit() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        call_contract(&env, &contract_id, || {
            Credit::draw_credit(env.clone(), borrower.clone(), 500_i128);
        });
        
        let credit_data = get_credit_data(&env, &contract_id, &borrower);
        assert_eq!(credit_data.utilized_amount, 500_i128);
        
        // Events are emitted - functionality verified through storage changes
    }

    #[test]
    fn test_repay_credit_partial() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        // First draw some credit
        call_contract(&env, &contract_id, || {
            Credit::draw_credit(env.clone(), borrower.clone(), 500_i128);
        });
        assert_eq!(get_credit_data(&env, &contract_id, &borrower).utilized_amount, 500_i128);
        
        // Partial repayment
        call_contract(&env, &contract_id, || {
            Credit::repay_credit(env.clone(), borrower.clone(), 200_i128);
        });
        
        let credit_data = get_credit_data(&env, &contract_id, &borrower);
        assert_eq!(credit_data.utilized_amount, 300_i128); // 500 - 200
    }

    #[test]
    fn test_repay_credit_full() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        // Draw some credit
        call_contract(&env, &contract_id, || {
            Credit::draw_credit(env.clone(), borrower.clone(), 500_i128);
        });
        assert_eq!(get_credit_data(&env, &contract_id, &borrower).utilized_amount, 500_i128);
        
        // Full repayment
        call_contract(&env, &contract_id, || {
            Credit::repay_credit(env.clone(), borrower.clone(), 500_i128);
        });
        
        let credit_data = get_credit_data(&env, &contract_id, &borrower);
        assert_eq!(credit_data.utilized_amount, 0_i128); // Fully repaid
    }

    #[test]
    fn test_repay_credit_overpayment() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        // Draw some credit
        call_contract(&env, &contract_id, || {
            Credit::draw_credit(env.clone(), borrower.clone(),300_i128);
        });
        assert_eq!(get_credit_data(&env, &contract_id, &borrower).utilized_amount, 300_i128);
        
        // Overpayment (pay more than utilized)
        call_contract(&env, &contract_id, || {
            Credit::repay_credit(env.clone(), borrower.clone(),500_i128);
        });
        
        let credit_data = get_credit_data(&env, &contract_id, &borrower);
        assert_eq!(credit_data.utilized_amount, 0_i128); // Should be capped at 0
    }

    #[test]
    fn test_repay_credit_zero_utilization() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        // Try to repay when no credit is utilized
        call_contract(&env, &contract_id, || {
            Credit::repay_credit(env.clone(), borrower.clone(),100_i128);
        });
        
        let credit_data = get_credit_data(&env, &contract_id, &borrower);
        assert_eq!(credit_data.utilized_amount, 0_i128); // Should remain 0
        
    }

    #[test]
    fn test_repay_credit_suspended_status() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        // Draw some credit
        call_contract(&env, &contract_id, || {
            Credit::draw_credit(env.clone(), borrower.clone(),500_i128);
        });
        
        // Manually set status to Suspended
        let credit_key = (Symbol::new(&env, "CREDIT_LINE"), borrower.clone());
        let mut credit_data = get_credit_data(&env, &contract_id, &borrower);
        credit_data.status = CreditStatus::Suspended;
        env.as_contract(&contract_id, || {
            env.storage().persistent().set(&credit_key, &credit_data);
        });
        
        // Should be able to repay even when suspended
        call_contract(&env, &contract_id, || {
            Credit::repay_credit(env.clone(), borrower.clone(),200_i128);
        });
        
        let updated_data = get_credit_data(&env, &contract_id, &borrower);
        assert_eq!(updated_data.utilized_amount, 300_i128);
        assert_eq!(updated_data.status, CreditStatus::Suspended); // Status should remain Suspended
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3)")]
    fn test_repay_credit_invalid_amount_zero() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        call_contract(&env, &contract_id, || {
            Credit::repay_credit(env.clone(), borrower.clone(),0_i128);
        });
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3)")]
    fn test_repay_credit_invalid_amount_negative() {
        let env = Env::default();
        let (_admin, borrower, contract_id) = setup_test(&env);
        
        call_contract(&env, &contract_id, || {
            Credit::repay_credit(env.clone(), borrower.clone(),-100_i128);
        });
    }

        client.init(&admin);
        client.open_credit_line(&borrower, &1000_i128, &300_u32, &70_u32);
        client.default_credit_line(&borrower);

        // Verify status changed to Defaulted
        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.status, CreditStatus::Defaulted);
    }

    #[test]
    fn test_full_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);

        // Open credit line
        client.open_credit_line(&borrower, &5000_i128, &500_u32, &80_u32);
        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.status, CreditStatus::Active);

        // Suspend credit line
        client.suspend_credit_line(&borrower);
        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.status, CreditStatus::Suspended);

        // Close credit line
        client.close_credit_line(&borrower);
        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.status, CreditStatus::Closed);
    }

    #[test]
    fn test_event_data_integrity() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.open_credit_line(&borrower, &2000_i128, &400_u32, &75_u32);

        // Verify credit line data matches what was passed
        let credit_line = client.get_credit_line(&borrower).unwrap();
        assert_eq!(credit_line.borrower, borrower);
        assert_eq!(credit_line.status, CreditStatus::Active);
        assert_eq!(credit_line.credit_limit, 2000);
        assert_eq!(credit_line.interest_rate_bps, 400);
        assert_eq!(credit_line.risk_score, 75);
    }

    #[test]
    #[should_panic(expected = "Credit line not found")]
    fn test_suspend_nonexistent_credit_line() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.suspend_credit_line(&borrower);
    }

    #[test]
    #[should_panic(expected = "Credit line not found")]
    fn test_close_nonexistent_credit_line() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.close_credit_line(&borrower);
    }

    #[test]
    #[should_panic(expected = "Credit line not found")]
    fn test_default_nonexistent_credit_line() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.default_credit_line(&borrower);
    }

    #[test]
    fn test_multiple_borrowers() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let borrower1 = Address::generate(&env);
        let borrower2 = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);
        client.open_credit_line(&borrower1, &1000_i128, &300_u32, &70_u32);
        client.open_credit_line(&borrower2, &2000_i128, &400_u32, &80_u32);

        let credit_line1 = client.get_credit_line(&borrower1).unwrap();
        let credit_line2 = client.get_credit_line(&borrower2).unwrap();

        assert_eq!(credit_line1.credit_limit, 1000);
        assert_eq!(credit_line2.credit_limit, 2000);
        assert_eq!(credit_line1.status, CreditStatus::Active);
        assert_eq!(credit_line2.status, CreditStatus::Active);
    }

    #[test]
    fn test_lifecycle_transitions() {
        let env = Env::default();
        env.mock_all_auths();
        
        let admin = Address::generate(&env);
        let borrower = Address::generate(&env);

        let contract_id = env.register(Credit, ());
        let client = CreditClient::new(&env, &contract_id);

        client.init(&admin);

        // Test Active -> Defaulted
        client.open_credit_line(&borrower, &1000_i128, &300_u32, &70_u32);
        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Active
        );

        client.default_credit_line(&borrower);
        assert_eq!(
            client.get_credit_line(&borrower).unwrap().status,
            CreditStatus::Defaulted
        );
    }
}
