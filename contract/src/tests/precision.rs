use crate::contract::{XRPL_DENOM_PREFIX, XRP_ISSUER};
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{
    CosmosTokensResponse, ExecuteMsg, PendingOperationsResponse, QueryMsg, XRPLTokensResponse,
};

use crate::state::{Config, CosmosToken, TokenState, XRPLToken};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{contract::XRP_CURRENCY, msg::InstantiateMsg, relayer::Relayer};
use cosmwasm_std::{coins, Addr, Uint128};
use token_bindings::{DenomUnit, Metadata};

#[test]
fn precisions() {
    let accounts_number = 2;
    let accounts: Vec<_> = (0..accounts_number)
        .into_iter()
        .map(|i| format!("account{i}"))
        .collect();

    let mut app = MockApp::new(&[
        (accounts[0].as_str(), &coins(100_000_000_000, FEE_DENOM)),
        (accounts[1].as_str(), &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let receiver = &accounts[1];

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
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 7,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();

    let config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    // *** Test with XRPL originated tokens ***

    let test_token1 = XRPLToken {
        issuer: generate_xrpl_address(),
        currency: "TT1".to_string(),
        sending_precision: -2,
        max_holding_amount: Uint128::new(200000000000000000),
        bridging_fee: Uint128::zero(),
        cosmos_denom: config.build_denom(&XRPL_DENOM_PREFIX.to_uppercase()),
        state: TokenState::Enabled,
    };
    let test_token2 = XRPLToken {
        issuer: generate_xrpl_address().to_string(),
        currency: "TT2".to_string(),
        sending_precision: 13,
        max_holding_amount: Uint128::new(499),
        bridging_fee: Uint128::zero(),
        cosmos_denom: config.build_denom(&XRPL_DENOM_PREFIX.to_uppercase()),
        state: TokenState::Enabled,
    };

    let test_token3 = XRPLToken {
        issuer: generate_xrpl_address().to_string(),
        currency: "TT3".to_string(),
        sending_precision: 0,
        max_holding_amount: Uint128::new(5000000000000000),
        bridging_fee: Uint128::zero(),
        cosmos_denom: config.build_denom(&XRPL_DENOM_PREFIX.to_uppercase()),
        state: TokenState::Enabled,
    };

    // Set up enough tickets first to allow registering tokens.
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(8),
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
                    tickets: Some((1..9).collect()),
                }),
            },
        },
        &[],
    )
    .unwrap();

    // Test negative sending precisions

    // Register token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token1.issuer.clone(),
            currency: test_token1.currency.clone(),
            sending_precision: test_token1.sending_precision.clone(),
            max_holding_amount: test_token1.max_holding_amount.clone(),
            bridging_fee: test_token1.bridging_fee,
        },
        &[],
    )
    .unwrap();

    let query_xrpl_tokens: XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let denom = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == test_token1.issuer && t.currency == test_token1.currency)
        .unwrap()
        .cosmos_denom
        .clone();

    // Activate the token
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token1.issuer.clone(),
                    currency: test_token1.currency.clone(),
                    // Sending less than 100000000000000000, in this case 99999999999999999 (1 less digit) should return an error because it will truncate to zero
                    amount: Uint128::new(99999999999999999),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: test_token1.issuer.clone(),
                currency: test_token1.currency.clone(),
                // Sending more than 199999999999999999 will truncate to 100000000000000000 and send it to the user and keep the remainder in the contract as fees to collect.
                amount: Uint128::new(199999999999999999),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(100000000000000000u128));

    // Sending anything again should not work because we already sent the maximum amount possible including the fees in the contract.
    let maximum_amount_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token1.issuer.clone(),
                    currency: test_token1.currency.clone(),
                    amount: Uint128::new(100000000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(maximum_amount_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    // Fees collected
    assert_eq!(request_balance, Uint128::from(99999999999999999u128));

    // Test positive sending precisions

    // Register token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token2.issuer.clone(),
            currency: test_token2.currency.clone(),
            sending_precision: test_token2.sending_precision.clone(),
            max_holding_amount: test_token2.max_holding_amount.clone(),
            bridging_fee: test_token2.bridging_fee,
        },
        &[],
    )
    .unwrap();

    // Activate the token
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    let query_xrpl_tokens: XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let denom = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == test_token2.issuer && t.currency == test_token2.currency)
        .unwrap()
        .cosmos_denom
        .clone();

    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    // Sending more than 499 should fail because maximum holding amount is 499
                    amount: Uint128::new(500),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    // Sending less than 100 will truncate to 0 so should fail
                    amount: Uint128::new(99),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: test_token2.issuer.clone(),
                currency: test_token2.currency.clone(),
                // Sending 299 should truncate the amount to 200 and keep the 99 in the contract as fees to collect
                amount: Uint128::new(299),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(200u128));

    // Sending 200 should work because we will reach exactly the maximum bridged amount.
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: test_token2.issuer.clone(),
                currency: test_token2.currency.clone(),
                amount: Uint128::new(200),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(400u128));

    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(99u128));

    // Sending anything again should fail because we passed the maximum bridged amount
    let maximum_amount_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    amount: Uint128::new(199),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(maximum_amount_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    // Test 0 sending precision

    // Register token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token3.issuer.clone(),
            currency: test_token3.currency.clone(),
            sending_precision: test_token3.sending_precision.clone(),
            max_holding_amount: test_token3.max_holding_amount.clone(),
            bridging_fee: test_token3.bridging_fee,
        },
        &[],
    )
    .unwrap();

    // Activate the token
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    let query_xrpl_tokens: XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let denom = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == test_token3.issuer && t.currency == test_token3.currency)
        .unwrap()
        .cosmos_denom
        .clone();

    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token3.issuer.clone(),
                    currency: test_token3.currency.clone(),
                    // Sending more than 5000000000000000 should fail because maximum holding amount is 5000000000000000
                    amount: Uint128::new(6000000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token3.issuer.clone(),
                    currency: test_token3.currency.clone(),
                    // Sending less than 1000000000000000 will truncate to 0 so should fail
                    amount: Uint128::new(900000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: test_token3.issuer.clone(),
                currency: test_token3.currency.clone(),
                // Sending 1111111111111111 should truncate the amount to 1000000000000000 and keep 111111111111111 as fees to collect
                amount: Uint128::new(1111111111111111),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(1000000000000000u128));

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: test_token3.issuer.clone(),
                currency: test_token3.currency.clone(),
                // Sending 3111111111111111 should truncate the amount to 3000000000000000 and keep another 111111111111111 as fees to collect
                amount: Uint128::new(3111111111111111),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(4000000000000000u128));

    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(222222222222222u128));

    let maximum_amount_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    // Sending 1111111111111111 should truncate the amount to 1000000000000000 and should fail because bridge is already holding maximum
                    amount: Uint128::new(1111111111111111),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(maximum_amount_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    // Test sending XRP
    let denom = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == XRP_ISSUER.to_string() && t.currency == XRP_CURRENCY.to_string())
        .unwrap()
        .cosmos_denom
        .clone();

    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: XRP_ISSUER.to_string(),
                    currency: XRP_CURRENCY.to_string(),
                    // Sending more than 100000000000000000 should fail because maximum holding amount is 10000000000000000 (1 less zero)
                    amount: Uint128::new(100000000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                // There should never be truncation because we allow full precision for XRP initially
                amount: Uint128::one(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(1u128));

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                // This should work because we are sending the rest to reach the maximum amount
                amount: Uint128::new(9999999999999999),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(10000000000000000u128));

    let maximum_amount_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: XRP_ISSUER.to_string(),
                    currency: XRP_CURRENCY.to_string(),
                    // Sending 1 more token would surpass the maximum so should fail
                    amount: Uint128::one(),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(maximum_amount_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    // *** Test with Orai originated tokens ***

    // Let's issue a few assets to the sender and registering them with different precisions and max sending amounts.

    for i in 1..=3 {
        let symbol = "TEST".to_string() + &i.to_string();
        let subunit = "utest".to_string() + &i.to_string();

        app.execute(
            Addr::unchecked(signer),
            token_factory_addr.clone(),
            &tokenfactory::msg::ExecuteMsg::CreateDenom {
                subdenom: subunit.to_uppercase(),
                metadata: Some(Metadata {
                    symbol: Some(symbol),
                    denom_units: vec![DenomUnit {
                        denom: subunit.clone(),
                        exponent: 6,
                        aliases: vec![],
                    }],
                    description: Some("description".to_string()),
                    base: None,
                    display: None,
                    name: None,
                }),
            },
            &[],
        )
        .unwrap();

        let denom = config.build_denom(&subunit.to_uppercase());

        app.execute(
            Addr::unchecked(signer),
            token_factory_addr.clone(),
            &tokenfactory::msg::ExecuteMsg::MintTokens {
                denom: denom.to_string(),
                amount: Uint128::from(100000000000000u128),
                mint_to_address: signer.to_string(),
            },
            &[],
        )
        .unwrap();
    }

    let denom1 = config.build_denom("UTEST1");
    let denom2 = config.build_denom("UTEST2");
    let denom3 = config.build_denom("UTEST3");

    let test_tokens = vec![
        CosmosToken {
            denom: denom1.clone(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::new(3),
            bridging_fee: Uint128::zero(),
            xrpl_currency: XRP_CURRENCY.to_string(),
            state: TokenState::Enabled,
        },
        CosmosToken {
            denom: denom2.clone(),
            decimals: 6,
            sending_precision: 0,
            max_holding_amount: Uint128::new(3990000),
            bridging_fee: Uint128::zero(),
            xrpl_currency: XRP_CURRENCY.to_string(),
            state: TokenState::Enabled,
        },
        CosmosToken {
            denom: denom3.clone(),
            decimals: 6,
            sending_precision: -6,
            max_holding_amount: Uint128::new(2000000000000),
            bridging_fee: Uint128::zero(),
            xrpl_currency: XRP_CURRENCY.to_string(),
            state: TokenState::Enabled,
        },
    ];

    // Register the tokens

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

    let query_cosmos_tokens: CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(query_cosmos_tokens.tokens.len(), 3);
    assert_eq!(query_cosmos_tokens.tokens[0].denom, test_tokens[0].denom);
    assert_eq!(query_cosmos_tokens.tokens[1].denom, test_tokens[1].denom);
    assert_eq!(query_cosmos_tokens.tokens[2].denom, test_tokens[2].denom);

    // Test sending token 1 with high precision

    // Sending 2 would work as it hasn't reached the maximum holding amount yet
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(2, denom1.clone()),
    )
    .unwrap();

    // Sending 1 more will hit max amount but will not fail
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(1, denom1.clone()),
    )
    .unwrap();

    // Trying to send 1 again would fail because we go over max bridge amount
    let maximum_amount_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1, denom1.clone()),
        )
        .unwrap_err();

    assert!(maximum_amount_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = app
        .query_balance(contract_addr.clone(), denom1.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(3u128));

    // Test sending token 2 with medium precision

    // Sending under sending precision would return error because it will be truncated to 0.
    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(100000, denom2.clone()),
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // Sending 3990000 would work as it is the maximum bridgable amount
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(3990000, denom2.clone()),
    )
    .unwrap();

    // Sending 100000 will fail because truncating will truncate to 0.
    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(100000, denom2.clone()),
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // Trying to send 1000000 would fail because we go over max bridge amount
    let maximum_amount_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1000000, denom2.clone()),
        )
        .unwrap_err();

    assert!(maximum_amount_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = app
        .query_balance(contract_addr.clone(), denom2.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(3990000u128));

    // Test sending token 3 with low precision

    // Sending 2000000000000 would work as it is the maximum bridgable amount
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(2000000000000, denom3.clone()),
    )
    .unwrap();

    // Sending 200000000000 (1 less zero) will fail because truncating will truncate to 0.
    let precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(200000000000, denom3.clone()),
        )
        .unwrap_err();

    assert!(precision_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // Trying to send 1000000000000 would fail because we go over max bridge amount
    let maximum_amount_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1000000000000, denom3.clone()),
        )
        .unwrap_err();

    assert!(maximum_amount_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = app
        .query_balance(contract_addr.clone(), denom3.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::from(2000000000000u128));
}
