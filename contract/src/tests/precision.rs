use crate::contract::{
    INITIAL_PROHIBITED_XRPL_ADDRESSES, MAX_RELAYERS, XRPL_DENOM_PREFIX, XRP_ISSUER, XRP_SYMBOL,
};
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{
    ExecuteMsg, OraiTokensResponse, PendingOperationsResponse, PendingRefundsResponse,
    ProcessedTxsResponse, QueryMsg, XRPLTokensResponse,
};
use crate::operation::{Operation, OperationType};
use crate::state::{Config, TokenState, XRPLToken};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{contract::XRP_CURRENCY, msg::InstantiateMsg, relayer::Relayer};
use cosmwasm_std::{coin, coins, Addr, BalanceResponse, BankMsg, SupplyResponse, Uint128};
use cosmwasm_testing_util::{BankSudo, Executor};
use token_bindings::{DenomUnit, FullDenomResponse, Metadata, MetadataResponse};

#[test]
fn precisions() {
    let app = OraiTestApp::new();
    let signer = app
        .init_account(&coins(100_000_000_000, FEE_DENOM))
        .unwrap();

    let receiver = app
        .init_account(&coins(100_000_000_000, FEE_DENOM))
        .unwrap();

    let relayer = Relayer {
        cosmos_address: Addr::unchecked("signer"),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let contract_addr = store_and_instantiate(
        &wasm,
        Addr::unchecked(signer),
        Addr::unchecked("signer"),
        vec![relayer],
        1,
        7,
        Uint128::new(TRUST_SET_LIMIT_AMOUNT),
        query_issue_fee(&asset_ft),
        generate_xrpl_address(),
        10,
    );

    // *** Test with XRPL originated tokens ***

    let test_token1 = XRPLToken {
        issuer: generate_xrpl_address(),
        currency: "TT1".to_string(),
        sending_precision: -2,
        max_holding_amount: Uint128::new(200000000000000000),
        bridging_fee: Uint128::zero(),
    };
    let test_token2 = XRPLToken {
        issuer: generate_xrpl_address().to_string(),
        currency: "TT2".to_string(),
        sending_precision: 13,
        max_holding_amount: Uint128::new(499),
        bridging_fee: Uint128::zero(),
    };

    let test_token3 = XRPLToken {
        issuer: generate_xrpl_address().to_string(),
        currency: "TT3".to_string(),
        sending_precision: 0,
        max_holding_amount: Uint128::new(5000000000000000),
        bridging_fee: Uint128::zero(),
    };

    // Set up enough tickets first to allow registering tokens.
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(8),
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    app.execute(
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
        Addr::unchecked(signer),
    )
    .unwrap();

    // Test negative sending precisions

    // Register token
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token1.issuer.clone(),
            currency: test_token1.currency.clone(),
            sending_precision: test_token1.sending_precision.clone(),
            max_holding_amount: test_token1.max_holding_amount.clone(),
            bridging_fee: test_token1.bridging_fee,
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let query_xrpl_tokens = wasm
        .query::<QueryMsg, XRPLTokensResponse>(
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
    let query_pending_operations = wasm
        .query::<QueryMsg, PendingOperationsResponse>(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    app.execute(
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
        Addr::unchecked(signer),
    )
    .unwrap();

    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token1.issuer.clone(),
                    currency: test_token1.currency.clone(),
                    // Sending less than 100000000000000000, in this case 99999999999999999 (1 less digit) should return an error because it will truncate to zero
                    amount: Uint128::new(99999999999999999),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: test_token1.issuer.clone(),
                currency: test_token1.currency.clone(),
                // Sending more than 199999999999999999 will truncate to 100000000000000000 and send it to the user and keep the remainder in the contract as fees to collect.
                amount: Uint128::new(199999999999999999),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            Addr::unchecked(receiver),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "100000000000000000".to_string());

    // Sending anything again should not work because we already sent the maximum amount possible including the fees in the contract.
    let maximum_amount_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token1.issuer.clone(),
                    currency: test_token1.currency.clone(),
                    amount: Uint128::new(100000000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(maximum_amount_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            contract_addr.clone(),
            denom: denom.clone(),
        })
        .unwrap();

    // Fees collected
    assert_eq!(request_balance.balance, "99999999999999999".to_string());

    // Test positive sending precisions

    // Register token
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token2.issuer.clone(),
            currency: test_token2.currency.clone(),
            sending_precision: test_token2.sending_precision.clone(),
            max_holding_amount: test_token2.max_holding_amount.clone(),
            bridging_fee: test_token2.bridging_fee,
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    // Activate the token
    let query_pending_operations = wasm
        .query::<QueryMsg, PendingOperationsResponse>(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    app.execute(
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
        Addr::unchecked(signer),
    )
    .unwrap();

    let query_xrpl_tokens = wasm
        .query::<QueryMsg, XRPLTokensResponse>(
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

    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    // Sending more than 499 should fail because maximum holding amount is 499
                    amount: Uint128::new(500),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    // Sending less than 100 will truncate to 0 so should fail
                    amount: Uint128::new(99),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: test_token2.issuer.clone(),
                currency: test_token2.currency.clone(),
                // Sending 299 should truncate the amount to 200 and keep the 99 in the contract as fees to collect
                amount: Uint128::new(299),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            Addr::unchecked(receiver),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "200".to_string());

    // Sending 200 should work because we will reach exactly the maximum bridged amount.
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: test_token2.issuer.clone(),
                currency: test_token2.currency.clone(),
                amount: Uint128::new(200),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            Addr::unchecked(receiver),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "400".to_string());

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            contract_addr.clone(),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "99".to_string());

    // Sending anything again should fail because we passed the maximum bridged amount
    let maximum_amount_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    amount: Uint128::new(199),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(maximum_amount_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    // Test 0 sending precision

    // Register token
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token3.issuer.clone(),
            currency: test_token3.currency.clone(),
            sending_precision: test_token3.sending_precision.clone(),
            max_holding_amount: test_token3.max_holding_amount.clone(),
            bridging_fee: test_token3.bridging_fee,
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    // Activate the token
    let query_pending_operations = wasm
        .query::<QueryMsg, PendingOperationsResponse>(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    app.execute(
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
        Addr::unchecked(signer),
    )
    .unwrap();

    let query_xrpl_tokens = wasm
        .query::<QueryMsg, XRPLTokensResponse>(
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

    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token3.issuer.clone(),
                    currency: test_token3.currency.clone(),
                    // Sending more than 5000000000000000 should fail because maximum holding amount is 5000000000000000
                    amount: Uint128::new(6000000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token3.issuer.clone(),
                    currency: test_token3.currency.clone(),
                    // Sending less than 1000000000000000 will truncate to 0 so should fail
                    amount: Uint128::new(900000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: test_token3.issuer.clone(),
                currency: test_token3.currency.clone(),
                // Sending 1111111111111111 should truncate the amount to 1000000000000000 and keep 111111111111111 as fees to collect
                amount: Uint128::new(1111111111111111),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            Addr::unchecked(receiver),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "1000000000000000".to_string());

    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: test_token3.issuer.clone(),
                currency: test_token3.currency.clone(),
                // Sending 3111111111111111 should truncate the amount to 3000000000000000 and keep another 111111111111111 as fees to collect
                amount: Uint128::new(3111111111111111),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            Addr::unchecked(receiver),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "4000000000000000".to_string());

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            contract_addr.clone(),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "222222222222222".to_string());

    let maximum_amount_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: test_token2.issuer.clone(),
                    currency: test_token2.currency.clone(),
                    // Sending 1111111111111111 should truncate the amount to 1000000000000000 and should fail because bridge is already holding maximum
                    amount: Uint128::new(1111111111111111),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(maximum_amount_error.to_string().contains(
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

    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: XRP_ISSUER.to_string(),
                    currency: XRP_CURRENCY.to_string(),
                    // Sending more than 100000000000000000 should fail because maximum holding amount is 10000000000000000 (1 less zero)
                    amount: Uint128::new(100000000000000000),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                // There should never be truncation because we allow full precision for XRP initially
                amount: Uint128::one(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            Addr::unchecked(receiver),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "1".to_string());

    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                // This should work because we are sending the rest to reach the maximum amount
                amount: Uint128::new(9999999999999999),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            Addr::unchecked(receiver),
            denom: denom.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "10000000000000000".to_string());

    let maximum_amount_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: generate_hash(),
                    issuer: XRP_ISSUER.to_string(),
                    currency: XRP_CURRENCY.to_string(),
                    // Sending 1 more token would surpass the maximum so should fail
                    amount: Uint128::one(),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(maximum_amount_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    // *** Test with Orai originated tokens ***

    // Let's issue a few assets to the sender and registering them with different precisions and max sending amounts.

    for i in 1..=3 {
        let symbol = "TEST".to_string() + &i.to_string();
        let subunit = "utest".to_string() + &i.to_string();
        asset_ft
            .issue(
                MsgIssue {
                    issuer: "signer",
                    symbol,
                    subunit,
                    precision: 6,
                    initial_amount: "100000000000000".to_string(),
                    description: "description".to_string(),
                    features: vec![MINTING as i32],
                    burn_rate: "0".to_string(),
                    send_commission_rate: "0".to_string(),
                    uri: "uri".to_string(),
                    uri_hash: "uri_hash".to_string(),
                },
                Addr::unchecked(signer),
            )
            .unwrap();
    }

    let denom1 = format!("{}-{}", "utest1", "signer").to_lowercase();
    let denom2 = format!("{}-{}", "utest2", "signer").to_lowercase();
    let denom3 = format!("{}-{}", "utest3", "signer").to_lowercase();

    let test_tokens = vec![
        OraiToken {
            denom: denom1.clone(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::new(3),
            bridging_fee: Uint128::zero(),
        },
        OraiToken {
            denom: denom2.clone(),
            decimals: 6,
            sending_precision: 0,
            max_holding_amount: Uint128::new(3990000),
            bridging_fee: Uint128::zero(),
        },
        OraiToken {
            denom: denom3.clone(),
            decimals: 6,
            sending_precision: -6,
            max_holding_amount: Uint128::new(2000000000000),
            bridging_fee: Uint128::zero(),
        },
    ];

    // Register the tokens

    for token in test_tokens.clone() {
        app.execute(
            contract_addr.clone(),
            &ExecuteMsg::RegisterOraiToken {
                denom: token.denom,
                decimals: token.decimals,
                sending_precision: token.sending_precision,
                max_holding_amount: token.max_holding_amount,
                bridging_fee: token.bridging_fee,
            },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap();
    }

    let query_cosmos_tokens = wasm
        .query::<QueryMsg, OraiTokensResponse>(
            contract_addr.clone(),
            &QueryMsg::OraiTokens {
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
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(2, denom1.clone()),
        Addr::unchecked(signer),
    )
    .unwrap();

    // Sending 1 more will hit max amount but will not fail
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(1, denom1.clone()),
        Addr::unchecked(signer),
    )
    .unwrap();

    // Trying to send 1 again would fail because we go over max bridge amount
    let maximum_amount_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1, denom1.clone()),
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(maximum_amount_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            contract_addr.clone(),
            denom: denom1.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "3".to_string());

    // Test sending token 2 with medium precision

    // Sending under sending precision would return error because it will be truncated to 0.
    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(100000, denom2.clone()),
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // Sending 3990000 would work as it is the maximum bridgable amount
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(3990000, denom2.clone()),
        Addr::unchecked(signer),
    )
    .unwrap();

    // Sending 100000 will fail because truncating will truncate to 0.
    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(100000, denom2.clone()),
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // Trying to send 1000000 would fail because we go over max bridge amount
    let maximum_amount_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1000000, denom2.clone()),
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(maximum_amount_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            contract_addr.clone(),
            denom: denom2.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "3990000".to_string());

    // Test sending token 3 with low precision

    // Sending 2000000000000 would work as it is the maximum bridgable amount
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(2000000000000, denom3.clone()),
        Addr::unchecked(signer),
    )
    .unwrap();

    // Sending 200000000000 (1 less zero) will fail because truncating will truncate to 0.
    let precision_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(200000000000, denom3.clone()),
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(precision_error.to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // Trying to send 1000000000000 would fail because we go over max bridge amount
    let maximum_amount_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1000000000000, denom3.clone()),
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(maximum_amount_error.to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    let request_balance = asset_ft
        .query_balance(&QueryBalanceRequest {
            contract_addr.clone(),
            denom: denom3.clone(),
        })
        .unwrap();

    assert_eq!(request_balance.balance, "2000000000000".to_string());
}
