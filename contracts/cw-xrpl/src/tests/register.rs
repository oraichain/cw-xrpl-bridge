use cosmwasm_std::{coins, Addr, Uint128};
use crate::contract::{MAX_COSMOS_TOKEN_DECIMALS, XRPL_DENOM_PREFIX};
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{PendingOperationsResponse, XRPLTokensResponse};
use crate::operation::{Operation, OperationType};
use crate::state::{  CosmosToken, XRPLToken};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM, TRUST_SET_LIMIT_AMOUNT
};
use crate::token::full_denom;
use crate::{
    contract::XRP_CURRENCY,
    msg::{
        CosmosTokensResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
    },
    relayer::Relayer,
    state::TokenState,
};
use token_bindings::DenomsByCreatorResponse;


#[test]
fn register_cosmos_token() {
    
    let (mut app,accounts) = MockApp::new(&[("signer", &coins(100_000_000_000, FEE_DENOM))]);
    let signer = &accounts[0];
    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();

    let test_tokens = vec![
        CosmosToken {
            denom: "denom1".to_string(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::new(100000),
            bridging_fee: Uint128::zero(),
            xrpl_currency: XRP_CURRENCY.to_string(),
            state: TokenState::Enabled,
        },
        CosmosToken {
            denom: "denom2".to_string(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::new(100000),
            bridging_fee: Uint128::zero(),
            xrpl_currency: XRP_CURRENCY.to_string(),
            state: TokenState::Enabled,
        },
    ];

    // Register two tokens correctly
    for token in test_tokens.clone() {
        app.execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterCosmosToken {
                denom: token.denom,
                decimals: token.decimals,
                sending_precision: token.sending_precision,
                max_holding_amount: token.max_holding_amount,
                bridging_fee: token.bridging_fee,
            },
            &[],
        )
        .unwrap();
    }

    // Registering a token with same denom, should fail    
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom:test_tokens[0].denom.clone(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::one(),
            bridging_fee: test_tokens[0].bridging_fee,
        },
        &[],
    )
    .unwrap_err();

    // Registering a token with invalid sending precision should fail
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: test_tokens[0].denom.clone(),
            decimals: 6,
            sending_precision: -17,
            max_holding_amount: Uint128::one(),
            bridging_fee: test_tokens[0].bridging_fee,
        },
        &[],
    )
    .unwrap_err();

    // Registering a token with invalid decimals should fail
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: test_tokens[0].denom.clone(),
            decimals: MAX_COSMOS_TOKEN_DECIMALS + 1,
            sending_precision: test_tokens[0].sending_precision,
            max_holding_amount: Uint128::one(),
            bridging_fee: test_tokens[0].bridging_fee,
        },
        &[],
    )
    .unwrap_err();

    // Registering tokens with invalid denoms will fail
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: "1aa".to_string(), // Starts with a number
            decimals: test_tokens[0].decimals,
            sending_precision: test_tokens[0].sending_precision,
            max_holding_amount: test_tokens[0].max_holding_amount,
            bridging_fee: test_tokens[0].bridging_fee,
        },
        &[],
    )
    .unwrap_err();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: "aa".to_string(), // Too short
            decimals: test_tokens[0].decimals,
            sending_precision: test_tokens[0].sending_precision,
            max_holding_amount: test_tokens[0].max_holding_amount,
            bridging_fee: test_tokens[0].bridging_fee,
        },
        &[],
    )
    .unwrap_err();

     app
            .execute(
                Addr::unchecked(signer),
                contract_addr.clone(),
                &ExecuteMsg::RegisterCosmosToken {
                    denom: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(), // Too long
                    decimals: test_tokens[0].decimals,
                    sending_precision: test_tokens[0].sending_precision,
                    max_holding_amount: test_tokens[0].max_holding_amount,
                    bridging_fee: test_tokens[0].bridging_fee,
                },
                &[],
                
            )
            .unwrap_err();

    

    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterCosmosToken {
                denom: "aa$".to_string(), // Invalid symbols
                decimals: test_tokens[0].decimals,
                sending_precision: test_tokens[0].sending_precision,
                max_holding_amount: test_tokens[0].max_holding_amount,
                bridging_fee: test_tokens[0].bridging_fee,
            },
            &[],
            
        )
        .unwrap_err();

    

    // Query all tokens
    let query_cosmos_tokens :CosmosTokensResponse= app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(query_cosmos_tokens.tokens.len(), 2);
    assert_eq!(query_cosmos_tokens.tokens[0].denom, test_tokens[0].denom);
    assert_eq!(query_cosmos_tokens.tokens[1].denom, test_tokens[1].denom);
    assert_eq!(
        query_cosmos_tokens.tokens[0].xrpl_currency,
        query_cosmos_tokens.tokens[0].xrpl_currency.to_uppercase()
    );
    assert_eq!(
        query_cosmos_tokens.tokens[1].xrpl_currency,
        query_cosmos_tokens.tokens[1].xrpl_currency.to_uppercase()
    );

    // Query tokens with limit
    let query_cosmos_tokens:CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(query_cosmos_tokens.tokens.len(), 1);
    assert_eq!(query_cosmos_tokens.tokens[0].denom, test_tokens[0].denom);

    // Query tokens with pagination
    let query_cosmos_tokens:CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: query_cosmos_tokens.last_key,
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(query_cosmos_tokens.tokens.len(), 1);
    assert_eq!(query_cosmos_tokens.tokens[0].denom, test_tokens[1].denom);
}


#[test]
fn register_xrpl_token() {
    
    let (mut app,accounts) = MockApp::new(&[("signer", &coins(100_000_000_000, FEE_DENOM))]);
    let signer = &accounts[0];
    
    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let xrpl_bridge_address = generate_xrpl_address();

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 2,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: xrpl_bridge_address.clone(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();
        

    let test_tokens = vec![
        XRPLToken {
            issuer: generate_xrpl_address(), // Valid issuer
            currency: "USD".to_string(),     // Valid standard currency code
            sending_precision: -15,
            max_holding_amount: Uint128::new(100),
            bridging_fee: Uint128::zero(),
            cosmos_denom:XRPL_DENOM_PREFIX.to_string(),
            state:TokenState::Enabled
        },
        XRPLToken {
            issuer: generate_xrpl_address(), // Valid issuer
            currency: "015841551A748AD2C1F76FF6ECB0CCCD00000000".to_string(), // Valid hexadecimal currency
            sending_precision: 15,
            max_holding_amount: Uint128::new(50000),
            bridging_fee: Uint128::zero(),
            cosmos_denom:XRPL_DENOM_PREFIX.to_string(),
            state:TokenState::Enabled
        },
    ];

    // Registering a token with an invalid issuer should fail.
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: "not_valid_issuer".to_string(),
                currency: test_tokens[0].currency.clone(),
                sending_precision: test_tokens[0].sending_precision.clone(),
                max_holding_amount: test_tokens[0].max_holding_amount.clone(),
                bridging_fee: test_tokens[0].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
        )
        .unwrap_err();


    // Registering a token with an invalid precision should fail.
     app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[0].issuer.clone(),
                currency: test_tokens[0].currency.clone(),
                sending_precision: -16,
                max_holding_amount: test_tokens[0].max_holding_amount.clone(),
                bridging_fee: test_tokens[0].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();


    // Registering a token with an invalid precision should fail.
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[0].issuer.clone(),
                currency: test_tokens[0].currency.clone(),
                sending_precision: 16,
                max_holding_amount: test_tokens[0].max_holding_amount.clone(),
                bridging_fee: test_tokens[0].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();


    // Registering a token with a valid issuer but invalid currency should fail.
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[1].issuer.clone(),
                currency: "invalid_currency".to_string(),
                sending_precision: test_tokens[1].sending_precision.clone(),
                max_holding_amount: test_tokens[1].max_holding_amount.clone(),
                bridging_fee: test_tokens[1].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Registering a token with an invalid symbol should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[1].issuer.clone(),
                currency: "US~".to_string(),
                sending_precision: test_tokens[1].sending_precision.clone(),
                max_holding_amount: test_tokens[1].max_holding_amount.clone(),
                bridging_fee: test_tokens[1].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Registering a token with an invalid hexadecimal currency (not uppercase) should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[1].issuer.clone(),
                currency: "015841551A748AD2C1f76FF6ECB0CCCD00000000".to_string(),
                sending_precision: test_tokens[1].sending_precision.clone(),
                max_holding_amount: test_tokens[1].max_holding_amount.clone(),
                bridging_fee: test_tokens[1].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Registering a token with an invalid hexadecimal currency (starting with 0x00) should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[1].issuer.clone(),
                currency: "005841551A748AD2C1F76FF6ECB0CCCD00000000".to_string(),
                sending_precision: test_tokens[1].sending_precision.clone(),
                max_holding_amount: test_tokens[1].max_holding_amount.clone(),
                bridging_fee: test_tokens[1].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Registering a token with an "XRP" as currency should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[1].issuer.clone(),
                currency: "XRP".to_string(),
                sending_precision: test_tokens[1].sending_precision.clone(),
                max_holding_amount: test_tokens[1].max_holding_amount.clone(),
                bridging_fee: test_tokens[1].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Register token with incorrect fee (too much), should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[0].issuer.clone(),
                currency: test_tokens[0].currency.clone(),
                sending_precision: test_tokens[0].sending_precision.clone(),
                max_holding_amount: test_tokens[0].max_holding_amount.clone(),
                bridging_fee: test_tokens[0].bridging_fee,
            },
            &coins(20_000_000, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Registering a token with an prohibited address as issuer should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: xrpl_bridge_address,
                currency: test_tokens[1].currency.clone(),
                sending_precision: test_tokens[1].sending_precision.clone(),
                max_holding_amount: test_tokens[1].max_holding_amount.clone(),
                bridging_fee: test_tokens[1].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Registering a token without having tickets for the TrustSet operation should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[0].issuer.clone(),
                currency: test_tokens[0].currency.clone(),
                sending_precision: test_tokens[0].sending_precision,
                max_holding_amount: test_tokens[0].max_holding_amount,
                bridging_fee: test_tokens[0].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Register two tokens correctly
    // Set up enough tickets first to allow registering tokens.
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(3),
        },
        &[],
        
    )
    .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..4).collect()),
                }),
            },
        },
        &[],
        
    )
    .unwrap();

    for token in test_tokens.clone() {
        app.execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: token.issuer,
                currency: token.currency,
                sending_precision: token.sending_precision,
                max_holding_amount: token.max_holding_amount,
                bridging_fee: token.bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap();
    }

    // Trying to register another token would fail because there is only 1 ticket left and that one is reserved
    let extra_token = XRPLToken {
        issuer: generate_xrpl_address(), // Valid issuer
        currency: "USD".to_string(),     // Valid standard currency code
        sending_precision: -15,
        max_holding_amount: Uint128::new(100),
        bridging_fee: Uint128::zero(),
        cosmos_denom:XRPL_DENOM_PREFIX.to_string(),
        state:TokenState::Enabled
    };

    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: extra_token.issuer,
                currency: extra_token.currency,
                sending_precision: extra_token.sending_precision,
                max_holding_amount: extra_token.max_holding_amount,
                bridging_fee: extra_token.bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    

    // Check tokens are in the bank module    
    let DenomsByCreatorResponse {denoms}= app.query(token_factory_addr.clone(), &tokenfactory::msg::QueryMsg::DenomsByCreator { creator: token_factory_addr.to_string() })        
        .unwrap();    

    assert_eq!(denoms.len(), 3);
    let denom_prefix = full_denom(&token_factory_addr, XRP_CURRENCY);    
    assert!(denoms[1]        
        .starts_with(&denom_prefix),);
    assert!(denoms[2]        
        .starts_with(&denom_prefix),);

    // Register 1 token with same issuer+currency, should fail
    app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: test_tokens[0].issuer.clone(),
                currency: test_tokens[0].currency.clone(),
                sending_precision: test_tokens[0].sending_precision.clone(),
                max_holding_amount: test_tokens[0].max_holding_amount.clone(),
                bridging_fee: test_tokens[0].bridging_fee,
            },
            &coins(10_000_000u128, FEE_DENOM),
            
        )
        .unwrap_err();

    // Query all tokens
    let query_xrpl_tokens:XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(query_xrpl_tokens.tokens.len(), 3);

    // Query all tokens with limit
    let query_xrpl_tokens:XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: Some(1),
            },
        )
        .unwrap();
    assert_eq!(query_xrpl_tokens.tokens.len(), 1);

    // Query all tokens with pagination
    let query_xrpl_tokens:XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: query_xrpl_tokens.last_key,
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(query_xrpl_tokens.tokens.len(), 2);
}

#[test]
fn xrpl_token_registration_recovery() {
    
    let (mut app,accounts) = MockApp::new(&[
        ("signer", &coins(100_000_000_000, FEE_DENOM)),
    ]);
    let signer = &accounts[0];


    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let token_issuer = generate_xrpl_address();
    let token_currency = "BTC".to_string();
    let xrpl_base_fee = 10;

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // Test with 1 relayer and 1 evidence threshold first
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 2,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address:generate_xrpl_address(),
                xrpl_base_fee,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();    

    
    let  issuer= token_issuer.clone();
    let  currency= token_currency.clone();
    let  sending_precision= -15;
    let  max_holding_amount= Uint128::new(100);
    let  bridging_fee= Uint128::zero();
        
    

    // We successfully recover 3 tickets to perform operations
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(3),
        },
        &[],
        
    )
    .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..4).collect()),
                }),
            },
        },
        &[],
        
    )
    .unwrap();

    // We perform the register token operation, which should put the token to Processing state and create the PendingOperation
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            sending_precision: sending_precision,
            max_holding_amount: max_holding_amount,
            bridging_fee: bridging_fee,
        },
        &coins(10_000_000u128, FEE_DENOM),
        
    )
    .unwrap();

    // If we try to recover a token that is not in Inactive state, it should fail.
    let recover_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RecoverXRPLTokenRegistration {
                issuer: issuer.clone(),
                currency: currency.clone(),
            },
            &[],
            
        )
        .unwrap_err();

    assert!(recover_error.root_cause()
        .to_string()
        .contains(ContractError::XRPLTokenNotInactive {}.to_string().as_str()));

    // If we try to recover a token that is not registered, it should fail
    let recover_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RecoverXRPLTokenRegistration {
                issuer: issuer.clone(),
                currency: "NOT".to_string(),
            },
            &[],
            
        )
        .unwrap_err();

    assert!(recover_error.root_cause()
        .to_string()
        .contains(ContractError::TokenNotRegistered {}.to_string().as_str()));

    // Let's fail the trust set operation to put the token to Inactive so that we can recover it

    let query_pending_operations :PendingOperationsResponse = app.query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 1);

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: Some(
                    query_pending_operations.operations[0]
                        .ticket_sequence
                        .unwrap(),
                ),
                transaction_result: TransactionResult::Rejected,
                operation_result: None,
            },
        },
        &[],
        
    )
    .unwrap();

    let query_pending_operations :PendingOperationsResponse = app.query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert!(query_pending_operations.operations.is_empty());

    // We should be able to recover the token now
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverXRPLTokenRegistration {
            issuer: issuer.clone(),
            currency: currency.clone(),
        },
        &[],
        
    )
    .unwrap();

    let query_pending_operations :PendingOperationsResponse = app.query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 1);
    assert_eq!(
        query_pending_operations.operations[0],
        Operation {
            id: query_pending_operations.operations[0].id.clone(),
            version: 1,
            ticket_sequence: Some(
                query_pending_operations.operations[0]
                    .ticket_sequence
                    .unwrap()
            ),
            account_sequence: None,
            signatures: vec![],
            operation_type: OperationType::TrustSet {
                issuer: token_issuer,
                currency: token_currency,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
            },
            xrpl_base_fee,
        }
    );
}
