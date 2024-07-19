use crate::contract::{MAX_RELAYERS, XRPL_DENOM_PREFIX, XRP_SYMBOL};
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{ExecuteMsg, PendingOperationsResponse, QueryMsg, XRPLTokensResponse};
use crate::state::{Config, TokenState, XRPLToken};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{contract::XRP_CURRENCY, msg::InstantiateMsg, relayer::Relayer};
use cosmwasm_std::{coin, coins, Addr, BankMsg, BankQuery, SupplyResponse, Uint128};
use cosmwasm_testing_util::{BankSudo, Executor};
use token_bindings::{DenomUnit, FullDenomResponse, Metadata, MetadataResponse};

#[test]
fn send_xrpl_originated_tokens_from_xrpl_to_coreum() {
    let accounts_number = 4;
    let accounts: Vec<_> = (0..accounts_number)
        .into_iter()
        .map(|i| format!("account{i}"))
        .collect();

    let mut app = MockApp::new(&[
        (accounts[0].as_str(), &coins(100_000_000_000, FEE_DENOM)),
        (accounts[1].as_str(), &coins(100_000_000_000, FEE_DENOM)),
        (accounts[2].as_str(), &coins(100_000_000_000, FEE_DENOM)),
        (accounts[3].as_str(), &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[accounts_number - 1];
    let receiver = &accounts[accounts_number - 2];
    let xrpl_addresses = vec![generate_xrpl_address(), generate_xrpl_address()];

    let xrpl_pub_keys = vec![generate_xrpl_pub_key(), generate_xrpl_pub_key()];

    let mut relayer_accounts = vec![];
    let mut relayers = vec![];

    for i in 0..accounts_number - 2 {
        let account = format!("account{}", i);
        relayer_accounts.push(account.clone());
        relayers.push(Relayer {
            coreum_address: Addr::unchecked(account),
            xrpl_address: xrpl_addresses[i].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i].to_string(),
        });
    }

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    let bridge_xrpl_address = generate_xrpl_address();

    // Test with 1 relayer and 1 evidence threshold first
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayers[0].clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 2,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address,
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
            },
        )
        .unwrap();

    let config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    let test_token = XRPLToken {
        issuer: generate_xrpl_address(),
        currency: "USD".to_string(),
        sending_precision: 15,
        max_holding_amount: Uint128::new(50000),
        bridging_fee: Uint128::zero(),
        coreum_denom: config.build_denom(XRPL_DENOM_PREFIX),
        state: TokenState::Enabled,
    };

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
        Addr::unchecked(&relayer_accounts[0]),
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

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token.issuer.clone(),
            currency: test_token.currency.clone(),
            sending_precision: test_token.sending_precision.clone(),
            max_holding_amount: test_token.max_holding_amount.clone(),
            bridging_fee: test_token.bridging_fee,
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
        .find(|t| t.issuer == test_token.issuer && t.currency == test_token.currency)
        .unwrap()
        .coreum_denom
        .clone();

    let hash = generate_hash();
    let amount = Uint128::new(100);

    // Bridging with 1 relayer before activating the token should return an error
    let not_active_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[0]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToOraiTransfer {
                    tx_hash: hash.clone(),
                    issuer: test_token.issuer.clone(),
                    currency: test_token.currency.clone(),
                    amount: amount.clone(),
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap_err();

    assert_eq!(
        not_active_error.root_cause().to_string(),
        ContractError::TokenNotEnabled {}.to_string()
    );

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

    assert_eq!(query_pending_operations.operations.len(), 1);

    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
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

    // Bridge with 1 relayer should immediately mint and send to the receiver address
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, amount);

    // If we try to bridge to the contract address, it should fail
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: generate_hash(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(contract_addr.clone()),
            },
        },
        &[],
    )
    .unwrap_err();

    // Test with more than 1 relayer
    // each token_factory will create a seperated namespace following its address
    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayers[0].clone(), relayers[1].clone()],
                evidence_threshold: 2,
                used_ticket_sequence_threshold: 2,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
            },
        )
        .unwrap();

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

    let hash2 = generate_hash();
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(hash2.clone()),
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

    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(hash2),
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

    // Register a token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: test_token.issuer.clone(),
            currency: test_token.currency.clone(),
            sending_precision: test_token.sending_precision,
            max_holding_amount: test_token.max_holding_amount,
            bridging_fee: test_token.bridging_fee,
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

    let tx_hash = generate_hash();
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(tx_hash.clone()),
                account_sequence: None,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(tx_hash),
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
        .find(|t| t.issuer == test_token.issuer && t.currency == test_token.currency)
        .unwrap()
        .coreum_denom
        .clone();

    // Trying to send from an address that is not a relayer should fail
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap_err();

    // Trying to send a token that is not previously registered should also fail
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: "not_registered".to_string(),
                currency: "not_registered".to_string(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap_err();

    // Trying to send invalid evidence should fail
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: Uint128::new(0),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap_err();

    // First relayer to execute should not trigger a mint and send
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    // Balance should be 0
    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::zero());

    // Relaying again from same relayer should trigger an error
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap_err();

    // Second relayer to execute should trigger a mint and send
    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap();

    // Balance should be 0
    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, amount);

    // Trying to relay again will trigger an error because operation is already executed
    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap_err();

    let new_amount = Uint128::new(150);
    // Trying to relay a different operation with same hash will trigger an error
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToOraiTransfer {
                tx_hash: hash.clone(),
                issuer: test_token.issuer.clone(),
                currency: test_token.currency.clone(),
                amount: new_amount.clone(),
                recipient: Addr::unchecked(receiver),
            },
        },
        &[],
    )
    .unwrap_err();
}

// #[test]
// fn send_coreum_originated_tokens_from_xrpl_to_coreum() {
//     let app = OraiTestApp::new();
//     let accounts_number = 3;
//     let accounts = app
//         .init_accounts(&coins(100_000_000_000, FEE_DENOM), accounts_number)
//         .unwrap();

//     let signer = accounts.get(0).unwrap();
//     let sender = accounts.get(1).unwrap();
//     let relayer_account = accounts.get(2).unwrap();
//     let relayer = Relayer {
//         coreum_address: Addr::unchecked("relayer"),
//         xrpl_address: generate_xrpl_address(),
//         xrpl_pub_key: generate_xrpl_pub_key(),
//     };

//     let xrpl_receiver_address = generate_xrpl_address();
//     let bridge_xrpl_address = generate_xrpl_address();

//     let contract_addr = store_and_instantiate(
//         &wasm,
//         signer,
//         Addr::unchecked("signer"),
//         vec![relayer.clone()],
//         1,
//         9,
//         Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//         query_issue_fee(&asset_ft),
//         bridge_xrpl_address.clone(),
//         10,
//     );

//     // Add enough tickets for all our test operations

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence: 1,
//             number_of_tickets: Some(10),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: Some(1),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: Some(OperationResult::TicketsAllocation {
//                     tickets: Some((1..11).collect()),
//                 }),
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Let's issue a token where decimals are less than an XRPL token decimals to the sender and register it.

//     let symbol = "TEST".to_string();
//     let subunit = "utest".to_string();
//     let decimals = 6;
//     let initial_amount = Uint128::new(100000000000000000000);
//     asset_ft
//         .issue(
//             MsgIssue {
//                 issuer: "signer",
//                 symbol,
//                 subunit: subunit.clone(),
//                 precision: decimals,
//                 initial_amount: initial_amount.to_string(),
//                 description: "description".to_string(),
//                 features: vec![MINTING as i32, FREEZING as i32],
//                 burn_rate: "0".to_string(),
//                 send_commission_rate: "0".to_string(),
//                 uri: "uri".to_string(),
//                 uri_hash: "uri_hash".to_string(),
//             },
//             Addr::unchecked(signer),
//         )
//         .unwrap();

//     let denom = format!("{}-{}", subunit, "signer").to_lowercase();

//     // Send all initial amount tokens to the sender so that we can correctly test freezing without sending to the issuer
//     let bank = Bank::new(&app);
//     bank.send(
//         MsgSend {
//             from_address: "signer",
//             to_address: sender.address(),
//             amount: vec![BaseCoin {
//                 amount: initial_amount.to_string(),
//                 denom: denom.to_string(),
//             }],
//         },
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterOraiToken {
//             denom: denom.clone(),
//             decimals,
//             sending_precision: 5,
//             max_holding_amount: Uint128::new(100000000000000000000),
//             bridging_fee: Uint128::zero(),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // It should truncate 1 because sending precision is 5
//     let amount_to_send = Uint128::new(1000001);

//     // If we try to send an amount in the optional field it should fail.
//     let send_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: xrpl_receiver_address.clone(),
//                 deliver_amount: Some(Uint128::new(100)),
//             },
//             &coins(amount_to_send.u128(), denom.clone()),
//             &sender,
//         )
//         .unwrap_err();

//     assert!(send_error.to_string().contains(
//         ContractError::DeliverAmountIsProhibited {}
//             .to_string()
//             .as_str()
//     ));

//     // If we try to send an amount that will become an invalid XRPL amount, it should fail
//     let send_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: xrpl_receiver_address.clone(),
//                 deliver_amount: None,
//             },
//             &coins(10000000000000000010, denom.clone()), // Nothing is truncated, and after transforming into XRPL amount it will have more than 17 digits
//             &sender,
//         )
//         .unwrap_err();

//     assert!(send_error
//         .to_string()
//         .contains(ContractError::InvalidXRPLAmount {}.to_string().as_str()));

//     // Try to bridge the token to the xrpl receiver address so that we can send it back.
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send.u128(), denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     // Check balance of sender and contract
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(amount_to_send)
//             .unwrap()
//             .to_string()
//     );

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, amount_to_send.to_string());

//     // Get the token information
//     let query_coreum_tokens = wasm
//         .query::<QueryMsg, OraiTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::OraiTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let coreum_originated_token = query_coreum_tokens
//         .tokens
//         .iter()
//         .find(|t| t.denom == denom)
//         .unwrap();

//     // Confirm the operation to remove it from pending operations.
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let amount_truncated_and_converted = Uint128::new(1000000000000000); // 100001 -> truncate -> 100000 -> convert -> 1e15
//     assert_eq!(query_pending_operations.operations.len(), 1);
//     assert_eq!(
//         query_pending_operations.operations[0].operation_type,
//         OperationType::OraiToXRPLTransfer {
//             issuer: bridge_xrpl_address.clone(),
//             currency: coreum_originated_token.xrpl_currency.clone(),
//             amount: amount_truncated_and_converted,
//             max_amount: Some(amount_truncated_and_converted),
//             sender: Addr::unchecked(sender.address()),
//             recipient: xrpl_receiver_address.clone(),
//         }
//     );

//     let tx_hash = generate_hash();
//     // Reject the operation, therefore the tokens should be stored in the pending refunds (except for truncated amount).
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(tx_hash.clone()),
//                 account_sequence: query_pending_operations.operations[0].account_sequence,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Truncated amount and amount to be refunded will stay in the contract until relayers and users to be refunded claim
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, amount_to_send.to_string());

//     // If we try to query pending refunds for any address that has no pending refunds, it should return an empty array
//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked("any_address"),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_refunds.pending_refunds, vec![]);

//     // Let's verify the pending refunds and try to claim them
//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
//     assert_eq!(
//         query_pending_refunds.pending_refunds[0].xrpl_tx_hash,
//         Some(tx_hash)
//     );
//     // Truncated amount (1) is not refundable
//     assert_eq!(
//         query_pending_refunds.pending_refunds[0].coin,
//         coin(
//             amount_to_send.checked_sub(Uint128::one()).unwrap().u128(),
//             denom.clone()
//         )
//     );

//     // Trying to claim a refund with an invalid pending refund operation id should fail
//     let claim_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::ClaimRefund {
//                 pending_refund_id: "random_id".to_string(),
//             },
//             &[],
//             &sender,
//         )
//         .unwrap_err();

//     assert!(claim_error
//         .to_string()
//         .contains(ContractError::PendingRefundNotFound {}.to_string().as_str()));

//     // Try to claim a pending refund with a valid pending refund operation id but not as a different user, should also fail
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ClaimRefund {
//             pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap_err();

//     // Let's freeze the token to verify that claiming will fail
//     asset_ft
//         .freeze(
//             MsgFreeze {
//                 sender: "signer",
//                 account: contract_addr.clone(),
//                 coin: Some(BaseCoin {
//                     denom: denom.clone(),
//                     amount: "100000".to_string(),
//                 }),
//             },
//             Addr::unchecked(signer),
//         )
//         .unwrap();

//     // Can't claim because token is frozen
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ClaimRefund {
//             pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
//         },
//         &[],
//         &sender,
//     )
//     .unwrap_err();

//     // Let's unfreeze token so we can claim
//     asset_ft
//         .unfreeze(
//             MsgUnfreeze {
//                 sender: "signer",
//                 account: contract_addr.clone(),
//                 coin: Some(BaseCoin {
//                     denom: denom.clone(),
//                     amount: "100000".to_string(),
//                 }),
//             },
//             Addr::unchecked(signer),
//         )
//         .unwrap();

//     // Let's claim our pending refund
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ClaimRefund {
//             pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
//         },
//         &[],
//         &sender,
//     )
//     .unwrap();

//     // Verify balance of sender (to check it was correctly refunded) and verify that the amount refunded was removed from pending refunds
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(Uint128::one()) // truncated amount
//             .unwrap()
//             .to_string()
//     );

//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     // We verify our pending refund operation was removed from the pending refunds
//     assert!(query_pending_refunds.pending_refunds.is_empty());

//     // Try to send again
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send.u128(), denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     // Send successfull evidence to remove from queue (tokens should be released on XRPL to the receiver)
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: query_pending_operations.operations[0].account_sequence,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 0);

//     // Test sending the amount back from XRPL to Orai
//     // 10000000000 (1e10) is the minimum we can send back (15 - 5 (sending precision))
//     let amount_to_send_back = Uint128::new(10000000000);

//     // If we send the token with a different issuer (not multisig address) it should fail
//     let transfer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: generate_xrpl_address(),
//                     currency: coreum_originated_token.xrpl_currency.clone(),
//                     amount: amount_to_send_back.clone(),
//                     recipient: Addr::unchecked(sender.address()),
//                 },
//             },
//             &[],
//             relayer_account,
//         )
//         .unwrap_err();

//     assert!(transfer_error
//         .to_string()
//         .contains(ContractError::TokenNotRegistered {}.to_string().as_str()));

//     // If we send the token with a different currency (one that is not the one in the registered token list) it should fail
//     let transfer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: bridge_xrpl_address.clone(),
//                     currency: "invalid_currency".to_string(),
//                     amount: amount_to_send_back.clone(),
//                     recipient: Addr::unchecked(sender.address()),
//                 },
//             },
//             &[],
//             relayer_account,
//         )
//         .unwrap_err();

//     assert!(transfer_error
//         .to_string()
//         .contains(ContractError::TokenNotRegistered {}.to_string().as_str()));

//     // Sending under the minimum should fail (minimum - 1)
//     let transfer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: bridge_xrpl_address.clone(),
//                     currency: coreum_originated_token.xrpl_currency.clone(),
//                     amount: amount_to_send_back.checked_sub(Uint128::one()).unwrap(),
//                     recipient: Addr::unchecked(sender.address()),
//                 },
//             },
//             &[],
//             relayer_account,
//         )
//         .unwrap_err();

//     assert!(transfer_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     // Sending the right evidence should move tokens from the contract to the sender's account
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: bridge_xrpl_address.clone(),
//                 currency: coreum_originated_token.xrpl_currency.clone(),
//                 amount: amount_to_send_back.clone(),
//                 recipient: Addr::unchecked(sender.address()),
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Check balance of sender and contract
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(amount_to_send) // initial amount
//             .unwrap()
//             .checked_sub(Uint128::one()) // amount lost during truncation of first rejection
//             .unwrap()
//             .checked_add(Uint128::new(10)) // Amount that we sent back (10) after conversion, the minimum
//             .unwrap()
//             .to_string()
//     );

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         amount_to_send
//             .checked_add(Uint128::one()) // Truncated amount staying in contract
//             .unwrap()
//             .checked_sub(Uint128::new(10))
//             .unwrap()
//             .to_string()
//     );

//     // Now let's issue a token where decimals are more than an XRPL token decimals to the sender and register it.
//     let symbol = "TEST2".to_string();
//     let subunit = "utest2".to_string();
//     let decimals = 20;
//     let initial_amount = Uint128::new(200000000000000000000); // 2e20
//     asset_ft
//         .issue(
//             MsgIssue {
//                 issuer: sender.address(),
//                 symbol,
//                 subunit: subunit.clone(),
//                 precision: decimals,
//                 initial_amount: initial_amount.to_string(),
//                 description: "description".to_string(),
//                 features: vec![MINTING as i32],
//                 burn_rate: "0".to_string(),
//                 send_commission_rate: "0".to_string(),
//                 uri: "uri".to_string(),
//                 uri_hash: "uri_hash".to_string(),
//             },
//             &sender,
//         )
//         .unwrap();

//     let denom = format!("{}-{}", subunit, sender.address()).to_lowercase();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterOraiToken {
//             denom: denom.clone(),
//             decimals,
//             sending_precision: 10,
//             max_holding_amount: Uint128::new(200000000000000000000), //2e20
//             bridging_fee: Uint128::zero(),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // It should truncate and remove all 9s because they are under precision
//     let amount_to_send = Uint128::new(100000000019999999999);

//     // Bridge the token to the xrpl receiver address so that we can send it back.
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send.u128(), denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     // Check balance of sender and contract
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(amount_to_send)
//             .unwrap()
//             .to_string()
//     );

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, amount_to_send.to_string());

//     // Get the token information
//     let query_coreum_tokens = wasm
//         .query::<QueryMsg, OraiTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::OraiTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let coreum_originated_token = query_coreum_tokens
//         .tokens
//         .iter()
//         .find(|t| t.denom == denom)
//         .unwrap();

//     // Confirm the operation to remove it from pending operations.
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let amount_truncated_and_converted = Uint128::new(1000000000100000); // 100000000019999999999 -> truncate -> 100000000010000000000  -> convert -> 1000000000100000
//     assert_eq!(query_pending_operations.operations.len(), 1);
//     assert_eq!(
//         query_pending_operations.operations[0].operation_type,
//         OperationType::OraiToXRPLTransfer {
//             issuer: bridge_xrpl_address.clone(),
//             currency: coreum_originated_token.xrpl_currency.clone(),
//             amount: amount_truncated_and_converted,
//             max_amount: Some(amount_truncated_and_converted),
//             sender: Addr::unchecked(sender.address()),
//             recipient: xrpl_receiver_address.clone(),
//         }
//     );

//     // Reject the operation so that tokens are sent back to sender
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: query_pending_operations.operations[0].account_sequence,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Truncated amount won't be sent back (goes to relayer fees) and the rest will be stored in refundable array for the user to claim
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(amount_to_send)
//             .unwrap()
//             .to_string()
//     );

//     // Truncated amount and refundable fees will stay in contract
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, amount_to_send.to_string());

//     // If we query the refundable tokens that the user can claim, we should see the amount that was truncated is claimable
//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     // We verify that these tokens are refundable
//     assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
//     assert_eq!(
//         query_pending_refunds.pending_refunds[0].coin,
//         coin(
//             amount_to_send
//                 .checked_sub(Uint128::new(9999999999)) // Amount truncated is not refunded to user
//                 .unwrap()
//                 .u128(),
//             denom.clone()
//         )
//     );

//     // Claim it, should work
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ClaimRefund {
//             pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
//         },
//         &[],
//         &sender,
//     )
//     .unwrap();

//     // pending refunds should now be empty
//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     // We verify that there are no pending refunds left
//     assert!(query_pending_refunds.pending_refunds.is_empty());

//     // Try to send again
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send.u128(), denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     // Send successfull evidence to remove from queue (tokens should be released on XRPL to the receiver)
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: query_pending_operations.operations[0].account_sequence,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 0);

//     // Test sending the amount back from XRPL to Orai
//     // 100000 (1e5) is the minimum we can send back (15 - 10 (sending precision))
//     let amount_to_send_back = Uint128::new(100000);

//     // If we send the token with a different issuer (not multisig address) it should fail
//     let transfer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: generate_xrpl_address(),
//                     currency: coreum_originated_token.xrpl_currency.clone(),
//                     amount: amount_to_send_back.clone(),
//                     recipient: Addr::unchecked(sender.address()),
//                 },
//             },
//             &[],
//             relayer_account,
//         )
//         .unwrap_err();

//     assert!(transfer_error
//         .to_string()
//         .contains(ContractError::TokenNotRegistered {}.to_string().as_str()));

//     // If we send the token with a different currency (one that is not the one in the registered token list) it should fail
//     let transfer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: bridge_xrpl_address.clone(),
//                     currency: "invalid_currency".to_string(),
//                     amount: amount_to_send_back.clone(),
//                     recipient: Addr::unchecked(sender.address()),
//                 },
//             },
//             &[],
//             relayer_account,
//         )
//         .unwrap_err();

//     assert!(transfer_error
//         .to_string()
//         .contains(ContractError::TokenNotRegistered {}.to_string().as_str()));

//     // Sending under the minimum should fail (minimum - 1)
//     let transfer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: bridge_xrpl_address.clone(),
//                     currency: coreum_originated_token.xrpl_currency.clone(),
//                     amount: amount_to_send_back.checked_sub(Uint128::one()).unwrap(),
//                     recipient: Addr::unchecked(sender.address()),
//                 },
//             },
//             &[],
//             relayer_account,
//         )
//         .unwrap_err();

//     assert!(transfer_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     // Sending the right evidence should move tokens from the contract to the sender's account
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: bridge_xrpl_address.clone(),
//                 currency: coreum_originated_token.xrpl_currency.clone(),
//                 amount: amount_to_send_back.clone(),
//                 recipient: Addr::unchecked(sender.address()),
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Check balance of sender and contract
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(amount_to_send) // initial amount
//             .unwrap()
//             .checked_sub(Uint128::new(9999999999)) // Amount lost during first truncation that was rejected
//             .unwrap()
//             .checked_add(Uint128::new(10000000000)) // Amount that we sent back after conversion (1e10), the minimum
//             .unwrap()
//             .to_string()
//     );

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         amount_to_send
//             .checked_add(Uint128::new(9999999999)) // Amount that was kept during truncation of rejected operation
//             .unwrap()
//             .checked_sub(Uint128::new(10000000000)) // Amount sent from XRPL to the user
//             .unwrap()
//             .to_string()
//     );
// }

// #[test]
// fn send_from_coreum_to_xrpl() {
//     let app = OraiTestApp::new();
//     let accounts_number = 3;
//     let accounts = app
//         .init_accounts(&coins(100_000_000_000, FEE_DENOM), accounts_number)
//         .unwrap();

//     let signer = accounts.get(0).unwrap();
//     let sender = accounts.get(1).unwrap();
//     let relayer_account = accounts.get(2).unwrap();
//     let relayer = Relayer {
//         coreum_address: Addr::unchecked("relayer"),
//         xrpl_address: generate_xrpl_address(),
//         xrpl_pub_key: generate_xrpl_pub_key(),
//     };

//     let xrpl_base_fee = 10;
//     let multisig_address = generate_xrpl_address();

//     let contract_addr = store_and_instantiate(
//         &wasm,
//         signer,
//         Addr::unchecked("signer"),
//         vec![relayer.clone()],
//         1,
//         10,
//         Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//         query_issue_fee(&asset_ft),
//         multisig_address.clone(),
//         xrpl_base_fee,
//     );

//     let query_xrpl_tokens = wasm
//         .query::<QueryMsg, XRPLTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::XRPLTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let denom_xrp = query_xrpl_tokens
//         .tokens
//         .iter()
//         .find(|t| t.issuer == XRP_ISSUER && t.currency == XRP_CURRENCY)
//         .unwrap()
//         .coreum_denom
//         .clone();

//     // Add enough tickets for all our test operations

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence: 1,
//             number_of_tickets: Some(11),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let tx_hash = generate_hash();
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(tx_hash.clone()),
//                 account_sequence: Some(1),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: Some(OperationResult::TicketsAllocation {
//                     tickets: Some((1..12).collect()),
//                 }),
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // If we query processed Txes with this tx_hash it should return true
//     let query_processed_tx = wasm
//         .query::<QueryMsg, bool>(
//             contract_addr.clone(),
//             &QueryMsg::ProcessedTx {
//                 hash: tx_hash.to_uppercase(),
//             },
//         )
//         .unwrap();

//     assert_eq!(query_processed_tx, true);

//     // If we query something that is not processed it should return false
//     let query_processed_tx = wasm
//         .query::<QueryMsg, bool>(
//             contract_addr.clone(),
//             &QueryMsg::ProcessedTx {
//                 hash: generate_hash(),
//             },
//         )
//         .unwrap();

//     assert_eq!(query_processed_tx, false);

//     // *** Test sending XRP back to XRPL, which is already enabled so we can bridge it directly ***

//     let amount_to_send_xrp = Uint128::new(50000);
//     let amount_to_send_back = Uint128::new(10000);
//     let final_balance_xrp = amount_to_send_xrp.checked_sub(amount_to_send_back).unwrap();
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: XRP_ISSUER.to_string(),
//                 currency: XRP_CURRENCY.to_string(),
//                 amount: amount_to_send_xrp.clone(),
//                 recipient: Addr::unchecked(sender.address()),
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Check that balance is in the sender's account
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrp.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, amount_to_send_xrp.to_string());

//     let xrpl_receiver_address = generate_xrpl_address();
//     // Trying to send XRP back with a deliver_amount should fail
//     let deliver_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: xrpl_receiver_address.clone(),
//                 deliver_amount: Some(Uint128::one()),
//             },
//             &coins(amount_to_send_back.u128(), denom_xrp.clone()),
//             sender,
//         )
//         .unwrap_err();

//     assert!(deliver_error.to_string().contains(
//         ContractError::DeliverAmountIsProhibited {}
//             .to_string()
//             .as_str()
//     ));

//     // Send the XRP back to XRPL successfully
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send_back.u128(), denom_xrp.clone()),
//         sender,
//     )
//     .unwrap();

//     // Check that operation is in the queue
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 1);
//     assert_eq!(
//         query_pending_operations.operations[0],
//         Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: Some(1),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::OraiToXRPLTransfer {
//                 issuer: XRP_ISSUER.to_string(),
//                 currency: XRP_CURRENCY.to_string(),
//                 amount: amount_to_send_back,
//                 max_amount: None,
//                 sender: Addr::unchecked(sender.address()),
//                 recipient: xrpl_receiver_address.clone(),
//             },
//             xrpl_base_fee,
//         }
//     );

//     // If we try to send tokens from Orai to XRPL using the multisig address as recipient, it should fail.
//     let bridge_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: multisig_address,
//                 deliver_amount: None,
//             },
//             &coins(1, denom_xrp.clone()),
//             sender,
//         )
//         .unwrap_err();

//     assert!(bridge_error
//         .to_string()
//         .contains(ContractError::ProhibitedAddress {}.to_string().as_str()));

//     // If we try to send tokens from Orai to XRPL using a prohibited address, it should fail.
//     let bridge_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: INITIAL_PROHIBITED_XRPL_ADDRESSES[0].to_string(),
//                 deliver_amount: None,
//             },
//             &coins(1, denom_xrp.clone()),
//             sender,
//         )
//         .unwrap_err();

//     assert!(bridge_error
//         .to_string()
//         .contains(ContractError::ProhibitedAddress {}.to_string().as_str()));

//     // Sending a OraiToXRPLTransfer evidence with account sequence should fail.
//     let invalid_evidence = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(generate_hash()),
//                     account_sequence: Some(1),
//                     ticket_sequence: None,
//                     transaction_result: TransactionResult::Accepted,
//                     operation_result: None,
//                 },
//             },
//             &[],
//             relayer_account,
//         )
//         .unwrap_err();

//     assert!(invalid_evidence.to_string().contains(
//         ContractError::InvalidTransactionResultEvidence {}
//             .to_string()
//             .as_str()
//     ));

//     // Send successful evidence to remove from queue (tokens should be released on XRPL to the receiver)
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(1),
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 0);

//     // Since transaction result was Accepted, the tokens must have been burnt
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrp.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, final_balance_xrp.to_string());

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom_xrp.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, Uint128::zero().to_string());

//     // Now we will try to send back again but this time reject it, thus balance must be sent back to the sender.

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send_back.u128(), denom_xrp.clone()),
//         sender,
//     )
//     .unwrap();

//     // Transaction was rejected
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(2),
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Since transaction result was Rejected, the tokens must have been sent to pending refunds

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom_xrp.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, amount_to_send_back.to_string());

//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     // We verify that these tokens are refundable
//     assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
//     assert_eq!(
//         query_pending_refunds.pending_refunds[0].coin,
//         coin(amount_to_send_back.u128(), denom_xrp.clone())
//     );

//     // *** Test sending an XRPL originated token back to XRPL ***

//     let test_token = XRPLToken {
//         issuer: generate_xrpl_address(),
//         currency: "TST".to_string(),
//         sending_precision: 15,
//         max_holding_amount: Uint128::new(50000000000000000000), // 5e20
//         bridging_fee: Uint128::zero(),
//     };

//     // First we need to register and activate it
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterXRPLToken {
//             issuer: test_token.issuer.clone(),
//             currency: test_token.currency.clone(),
//             sending_precision: test_token.sending_precision,
//             max_holding_amount: test_token.max_holding_amount,
//             bridging_fee: test_token.bridging_fee,
//         },
//         &query_issue_fee(&asset_ft),
//         signer,
//     )
//     .unwrap();

//     // Activate the token
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let tx_hash = generate_hash();
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(tx_hash.clone()),
//                 account_sequence: None,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let amount_to_send = Uint128::new(10000000000000000000); // 1e20
//     let final_balance = amount_to_send.checked_sub(amount_to_send_back).unwrap();
//     // Bridge some tokens to the sender address
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: test_token.issuer.to_string(),
//                 currency: test_token.currency.to_string(),
//                 amount: amount_to_send.clone(),
//                 recipient: Addr::unchecked(sender.address()),
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let query_xrpl_tokens = wasm
//         .query::<QueryMsg, XRPLTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::XRPLTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let xrpl_originated_token = query_xrpl_tokens
//         .tokens
//         .iter()
//         .find(|t| t.issuer == test_token.issuer && t.currency == test_token.currency)
//         .unwrap();
//     let denom_xrpl_origin_token = xrpl_originated_token.coreum_denom.clone();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrpl_origin_token.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, amount_to_send.to_string());

//     // If we send more than one token in the funds we should get an error
//     let invalid_funds_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: xrpl_receiver_address.clone(),
//                 deliver_amount: None,
//             },
//             &vec![
//                 coin(1, FEE_DENOM),
//                 coin(amount_to_send_back.u128(), denom_xrpl_origin_token.clone()),
//             ],
//             sender,
//         )
//         .unwrap_err();

//     assert!(invalid_funds_error.to_string().contains(
//         ContractError::Payment(cw_utils::PaymentError::MultipleDenoms {})
//             .to_string()
//             .as_str()
//     ));

//     // If we send to an invalid XRPL address we should get an error
//     let invalid_address_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: "invalid_address".to_string(),
//                 deliver_amount: None,
//             },
//             &coins(amount_to_send_back.u128(), denom_xrpl_origin_token.clone()),
//             sender,
//         )
//         .unwrap_err();

//     assert!(invalid_address_error.to_string().contains(
//         ContractError::InvalidXRPLAddress {
//             address: "invalid_address".to_string()
//         }
//         .to_string()
//         .as_str()
//     ));

//     // We will send a successful transfer to XRPL considering the token has no transfer rate

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send_back.u128(), denom_xrpl_origin_token.clone()),
//         sender,
//     )
//     .unwrap();

//     // Check that the operation was added to the queue

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 1);
//     assert_eq!(
//         query_pending_operations.operations[0],
//         Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: Some(4),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::OraiToXRPLTransfer {
//                 issuer: xrpl_originated_token.issuer.clone(),
//                 currency: xrpl_originated_token.currency.clone(),
//                 amount: amount_to_send_back,
//                 max_amount: Some(amount_to_send_back),
//                 sender: Addr::unchecked(sender.address()),
//                 recipient: xrpl_receiver_address.clone(),
//             },
//             xrpl_base_fee
//         }
//     );

//     // Send successful should burn the tokens
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(4),
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 0);

//     // Tokens should have been burnt since transaction was accepted
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrpl_origin_token.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, final_balance.to_string());

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom_xrpl_origin_token.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, Uint128::zero().to_string());

//     // Now we will try to send back again but this time reject it, thus balance must be sent back to the sender
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send_back.u128(), denom_xrpl_origin_token.clone()),
//         sender,
//     )
//     .unwrap();

//     // Send rejected should store tokens minus truncated amount in refundable amount for the sender
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(5),
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrp.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         final_balance_xrp
//             .checked_sub(amount_to_send_back)
//             .unwrap()
//             .to_string()
//     );

//     // Let's check the pending refunds for the sender and also check that pagination works correctly.
//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     // There was one pending refund from previous test, we are going to claim both
//     assert_eq!(query_pending_refunds.pending_refunds.len(), 2);

//     // Test with limit 1 and starting after first one
//     let query_pending_refunds_with_limit = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: Some(1),
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_refunds_with_limit.pending_refunds.len(), 1);

//     // Test with limit 1 and starting from first key
//     let query_pending_refunds_with_limit_and_start_after_key = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: query_pending_refunds_with_limit.last_key,
//                 limit: Some(1),
//             },
//         )
//         .unwrap();

//     assert_eq!(
//         query_pending_refunds_with_limit_and_start_after_key
//             .pending_refunds
//             .len(),
//         1
//     );
//     assert_eq!(
//         query_pending_refunds_with_limit_and_start_after_key.pending_refunds[0],
//         query_pending_refunds.pending_refunds[1]
//     );

//     // Let's claim all pending refunds and check that they are gone from the contract and in the senders address
//     for refund in query_pending_refunds.pending_refunds.iter() {
//         app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::ClaimRefund {
//                 pending_refund_id: refund.id.clone(),
//             },
//             &[],
//             &sender,
//         )
//         .unwrap();
//     }

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrp.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, final_balance_xrp.to_string());

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom_xrp.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, Uint128::zero().to_string());

//     // Let's test sending a token with optional amount

//     let max_amount = Uint128::new(9999999999999999);
//     let deliver_amount = Some(Uint128::new(6000));

//     // Store balance first so we can check it later
//     let request_initial_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrpl_origin_token.clone(),
//         })
//         .unwrap();

//     // If we send amount that is higher than max amount, it should fail
//     let max_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: xrpl_receiver_address.clone(),
//                 deliver_amount: Some(max_amount.checked_add(Uint128::one()).unwrap()),
//             },
//             &coins(max_amount.u128(), denom_xrpl_origin_token.clone()),
//             sender,
//         )
//         .unwrap_err();

//     assert!(max_amount_error
//         .to_string()
//         .contains(ContractError::InvalidDeliverAmount {}.to_string().as_str()));

//     // If we send a deliver amount that is an invalid XRPL amount, it should fail
//     let invalid_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: xrpl_receiver_address.clone(),
//                 deliver_amount: Some(Uint128::new(99999999999999999)),
//             },
//             &coins(1000000000000000000, denom_xrpl_origin_token.clone()),
//             sender,
//         )
//         .unwrap_err();

//     assert!(invalid_amount_error
//         .to_string()
//         .contains(ContractError::InvalidXRPLAmount {}.to_string().as_str()));

//     // If we send an amount that is an invalid XRPL amount, it should fail
//     let invalid_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: xrpl_receiver_address.clone(),
//                 deliver_amount: Some(Uint128::new(10000000000000000)),
//             },
//             &coins(10000000000000001, denom_xrpl_origin_token.clone()),
//             sender,
//         )
//         .unwrap_err();

//     assert!(invalid_amount_error
//         .to_string()
//         .contains(ContractError::InvalidXRPLAmount {}.to_string().as_str()));

//     // Send it correctly
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount,
//         },
//         &coins(max_amount.u128(), denom_xrpl_origin_token.clone()),
//         sender,
//     )
//     .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 1);
//     assert_eq!(
//         query_pending_operations.operations[0],
//         Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: Some(6),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::OraiToXRPLTransfer {
//                 issuer: xrpl_originated_token.issuer.clone(),
//                 currency: xrpl_originated_token.currency.clone(),
//                 amount: deliver_amount.unwrap(),
//                 max_amount: Some(max_amount),
//                 sender: Addr::unchecked(sender.address()),
//                 recipient: xrpl_receiver_address.clone(),
//             },
//             xrpl_base_fee
//         }
//     );

//     // If we reject the operation, the refund should be stored for the amount of funds that were sent
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(6),
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Check balances and pending refunds are all correct
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrpl_origin_token.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         request_initial_balance
//             .balance
//             .parse::<u128>()
//             .unwrap()
//             .checked_sub(max_amount.u128())
//             .unwrap()
//             .to_string()
//     );

//     // Let's check the pending refunds for the sender and also check that pagination works correctly.
//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
//     assert_eq!(
//         query_pending_refunds.pending_refunds[0].coin,
//         coin(max_amount.u128(), denom_xrpl_origin_token.clone())
//     );

//     // Claim it back

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ClaimRefund {
//             pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
//         },
//         &[],
//         &sender,
//     )
//     .unwrap();

//     // Check balance again
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom_xrpl_origin_token.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, request_initial_balance.balance);

//     // *** Test sending Orai originated tokens to XRPL

//     // Let's issue a token to the sender and register it.

//     let symbol = "TEST".to_string();
//     let subunit = "utest".to_string();
//     let initial_amount = Uint128::new(1000000000);
//     let decimals = 6;
//     asset_ft
//         .issue(
//             MsgIssue {
//                 issuer: sender.address(),
//                 symbol,
//                 subunit: subunit.clone(),
//                 precision: decimals,
//                 initial_amount: initial_amount.to_string(),
//                 description: "description".to_string(),
//                 features: vec![MINTING as i32],
//                 burn_rate: "0".to_string(),
//                 send_commission_rate: "0".to_string(),
//                 uri: "uri".to_string(),
//                 uri_hash: "uri_hash".to_string(),
//             },
//             &sender,
//         )
//         .unwrap();

//     let denom = format!("{}-{}", subunit, sender.address()).to_lowercase();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterOraiToken {
//             denom: denom.clone(),
//             decimals,
//             sending_precision: 5,
//             max_holding_amount: Uint128::new(10000000),
//             bridging_fee: Uint128::zero(),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let amount_to_send = Uint128::new(1000001); // 1000001 -> truncate -> 1e6 -> decimal conversion -> 1e15

//     // Bridge the token to the xrpl receiver address two times and check pending operations
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send.u128(), denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(amount_to_send.u128(), denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     let multisig_address = wasm
//         .query::<QueryMsg, Config>(contract_addr.clone(), &QueryMsg::Config {})
//         .unwrap()
//         .bridge_xrpl_address;

//     let query_coreum_tokens = wasm
//         .query::<QueryMsg, OraiTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::OraiTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let coreum_originated_token = query_coreum_tokens
//         .tokens
//         .iter()
//         .find(|t| t.denom == denom)
//         .unwrap();

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 2);
//     let amount = amount_to_send
//         .checked_sub(Uint128::one()) //Truncated amount
//         .unwrap()
//         .checked_mul(Uint128::new(10u128.pow(9))) // XRPL Decimals - Orai Decimals -> (15 - 6) = 9
//         .unwrap();
//     assert_eq!(
//         query_pending_operations.operations[0],
//         Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: Some(7),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::OraiToXRPLTransfer {
//                 issuer: multisig_address.clone(),
//                 currency: coreum_originated_token.xrpl_currency.clone(),
//                 amount: amount.clone(),
//                 max_amount: Some(amount.clone()),
//                 sender: Addr::unchecked(sender.address()),
//                 recipient: xrpl_receiver_address.clone(),
//             },
//             xrpl_base_fee
//         }
//     );

//     assert_eq!(
//         query_pending_operations.operations[1],
//         Operation {
//             id: query_pending_operations.operations[1].id.clone(),
//             version: 1,
//             ticket_sequence: Some(8),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::OraiToXRPLTransfer {
//                 issuer: multisig_address,
//                 currency: coreum_originated_token.xrpl_currency.clone(),
//                 amount: amount.clone(),
//                 max_amount: Some(amount.clone()),
//                 sender: Addr::unchecked(sender.address()),
//                 recipient: xrpl_receiver_address,
//             },
//             xrpl_base_fee
//         }
//     );

//     // If we reject both operations, the tokens should be kept in pending refunds with different ids for the sender to claim (except truncated amount)
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(7),
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(8),
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     // Refundable amount (amount to send x 2 - truncated amount x 2) won't be sent back until claimed individually
//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(amount_to_send)
//             .unwrap()
//             .checked_sub(amount_to_send)
//             .unwrap()
//             .to_string()
//     );

//     let query_pending_refunds = wasm
//         .query::<QueryMsg, PendingRefundsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingRefunds {
//                 address: Addr::unchecked(sender.address()),
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_refunds.pending_refunds.len(), 2);

//     // Claiming pending refund should work for both operations
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ClaimRefund {
//             pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
//         },
//         &[],
//         &sender,
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ClaimRefund {
//             pending_refund_id: query_pending_refunds.pending_refunds[1].id.clone(),
//         },
//         &[],
//         &sender,
//     )
//     .unwrap();

//     // Check that balance was correctly sent back
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: sender.address(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(
//         request_balance.balance,
//         initial_amount
//             .checked_sub(Uint128::new(2))
//             .unwrap()
//             .to_string()
//     );

//     // Truncated amount will stay in contract
//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();
//     assert_eq!(request_balance.balance, Uint128::new(2).to_string());

//     // Let's query all processed transactions
//     let query_processed_txs = wasm
//         .query::<QueryMsg, ProcessedTxsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::ProcessedTxs {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_processed_txs.processed_txs.len(), 11);

//     // Let's query with pagination
//     let query_processed_txs = wasm
//         .query::<QueryMsg, ProcessedTxsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::ProcessedTxs {
//                 start_after_key: None,
//                 limit: Some(4),
//             },
//         )
//         .unwrap();

//     assert_eq!(query_processed_txs.processed_txs.len(), 4);

//     let query_processed_txs = wasm
//         .query::<QueryMsg, ProcessedTxsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::ProcessedTxs {
//                 start_after_key: query_processed_txs.last_key,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_processed_txs.processed_txs.len(), 7);
// }

// #[test]
// fn precisions() {
//     let app = OraiTestApp::new();
//     let signer = app
//         .init_account(&coins(100_000_000_000, FEE_DENOM))
//         .unwrap();

//     let receiver = app
//         .init_account(&coins(100_000_000_000, FEE_DENOM))
//         .unwrap();

//     let relayer = Relayer {
//         coreum_address: Addr::unchecked("signer"),
//         xrpl_address: generate_xrpl_address(),
//         xrpl_pub_key: generate_xrpl_pub_key(),
//     };

//     let contract_addr = store_and_instantiate(
//         &wasm,
//         Addr::unchecked(signer),
//         Addr::unchecked("signer"),
//         vec![relayer],
//         1,
//         7,
//         Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//         query_issue_fee(&asset_ft),
//         generate_xrpl_address(),
//         10,
//     );

//     // *** Test with XRPL originated tokens ***

//     let test_token1 = XRPLToken {
//         issuer: generate_xrpl_address(),
//         currency: "TT1".to_string(),
//         sending_precision: -2,
//         max_holding_amount: Uint128::new(200000000000000000),
//         bridging_fee: Uint128::zero(),
//     };
//     let test_token2 = XRPLToken {
//         issuer: generate_xrpl_address().to_string(),
//         currency: "TT2".to_string(),
//         sending_precision: 13,
//         max_holding_amount: Uint128::new(499),
//         bridging_fee: Uint128::zero(),
//     };

//     let test_token3 = XRPLToken {
//         issuer: generate_xrpl_address().to_string(),
//         currency: "TT3".to_string(),
//         sending_precision: 0,
//         max_holding_amount: Uint128::new(5000000000000000),
//         bridging_fee: Uint128::zero(),
//     };

//     // Set up enough tickets first to allow registering tokens.
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence: 1,
//             number_of_tickets: Some(8),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: Some(1),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: Some(OperationResult::TicketsAllocation {
//                     tickets: Some((1..9).collect()),
//                 }),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Test negative sending precisions

//     // Register token
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterXRPLToken {
//             issuer: test_token1.issuer.clone(),
//             currency: test_token1.currency.clone(),
//             sending_precision: test_token1.sending_precision.clone(),
//             max_holding_amount: test_token1.max_holding_amount.clone(),
//             bridging_fee: test_token1.bridging_fee,
//         },
//         &query_issue_fee(&asset_ft),
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let query_xrpl_tokens = wasm
//         .query::<QueryMsg, XRPLTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::XRPLTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let denom = query_xrpl_tokens
//         .tokens
//         .iter()
//         .find(|t| t.issuer == test_token1.issuer && t.currency == test_token1.currency)
//         .unwrap()
//         .coreum_denom
//         .clone();

//     // Activate the token
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token1.issuer.clone(),
//                     currency: test_token1.currency.clone(),
//                     // Sending less than 100000000000000000, in this case 99999999999999999 (1 less digit) should return an error because it will truncate to zero
//                     amount: Uint128::new(99999999999999999),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: test_token1.issuer.clone(),
//                 currency: test_token1.currency.clone(),
//                 // Sending more than 199999999999999999 will truncate to 100000000000000000 and send it to the user and keep the remainder in the contract as fees to collect.
//                 amount: Uint128::new(199999999999999999),
//                 recipient: Addr::unchecked(receiver),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             Addr::unchecked(receiver),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "100000000000000000".to_string());

//     // Sending anything again should not work because we already sent the maximum amount possible including the fees in the contract.
//     let maximum_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token1.issuer.clone(),
//                     currency: test_token1.currency.clone(),
//                     amount: Uint128::new(100000000000000000),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(maximum_amount_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     // Fees collected
//     assert_eq!(request_balance.balance, "99999999999999999".to_string());

//     // Test positive sending precisions

//     // Register token
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterXRPLToken {
//             issuer: test_token2.issuer.clone(),
//             currency: test_token2.currency.clone(),
//             sending_precision: test_token2.sending_precision.clone(),
//             max_holding_amount: test_token2.max_holding_amount.clone(),
//             bridging_fee: test_token2.bridging_fee,
//         },
//         &query_issue_fee(&asset_ft),
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Activate the token
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let query_xrpl_tokens = wasm
//         .query::<QueryMsg, XRPLTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::XRPLTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let denom = query_xrpl_tokens
//         .tokens
//         .iter()
//         .find(|t| t.issuer == test_token2.issuer && t.currency == test_token2.currency)
//         .unwrap()
//         .coreum_denom
//         .clone();

//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token2.issuer.clone(),
//                     currency: test_token2.currency.clone(),
//                     // Sending more than 499 should fail because maximum holding amount is 499
//                     amount: Uint128::new(500),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token2.issuer.clone(),
//                     currency: test_token2.currency.clone(),
//                     // Sending less than 100 will truncate to 0 so should fail
//                     amount: Uint128::new(99),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: test_token2.issuer.clone(),
//                 currency: test_token2.currency.clone(),
//                 // Sending 299 should truncate the amount to 200 and keep the 99 in the contract as fees to collect
//                 amount: Uint128::new(299),
//                 recipient: Addr::unchecked(receiver),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             Addr::unchecked(receiver),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "200".to_string());

//     // Sending 200 should work because we will reach exactly the maximum bridged amount.
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: test_token2.issuer.clone(),
//                 currency: test_token2.currency.clone(),
//                 amount: Uint128::new(200),
//                 recipient: Addr::unchecked(receiver),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             Addr::unchecked(receiver),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "400".to_string());

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "99".to_string());

//     // Sending anything again should fail because we passed the maximum bridged amount
//     let maximum_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token2.issuer.clone(),
//                     currency: test_token2.currency.clone(),
//                     amount: Uint128::new(199),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(maximum_amount_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     // Test 0 sending precision

//     // Register token
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterXRPLToken {
//             issuer: test_token3.issuer.clone(),
//             currency: test_token3.currency.clone(),
//             sending_precision: test_token3.sending_precision.clone(),
//             max_holding_amount: test_token3.max_holding_amount.clone(),
//             bridging_fee: test_token3.bridging_fee,
//         },
//         &query_issue_fee(&asset_ft),
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Activate the token
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: None,
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let query_xrpl_tokens = wasm
//         .query::<QueryMsg, XRPLTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::XRPLTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let denom = query_xrpl_tokens
//         .tokens
//         .iter()
//         .find(|t| t.issuer == test_token3.issuer && t.currency == test_token3.currency)
//         .unwrap()
//         .coreum_denom
//         .clone();

//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token3.issuer.clone(),
//                     currency: test_token3.currency.clone(),
//                     // Sending more than 5000000000000000 should fail because maximum holding amount is 5000000000000000
//                     amount: Uint128::new(6000000000000000),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token3.issuer.clone(),
//                     currency: test_token3.currency.clone(),
//                     // Sending less than 1000000000000000 will truncate to 0 so should fail
//                     amount: Uint128::new(900000000000000),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: test_token3.issuer.clone(),
//                 currency: test_token3.currency.clone(),
//                 // Sending 1111111111111111 should truncate the amount to 1000000000000000 and keep 111111111111111 as fees to collect
//                 amount: Uint128::new(1111111111111111),
//                 recipient: Addr::unchecked(receiver),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             Addr::unchecked(receiver),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "1000000000000000".to_string());

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: test_token3.issuer.clone(),
//                 currency: test_token3.currency.clone(),
//                 // Sending 3111111111111111 should truncate the amount to 3000000000000000 and keep another 111111111111111 as fees to collect
//                 amount: Uint128::new(3111111111111111),
//                 recipient: Addr::unchecked(receiver),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             Addr::unchecked(receiver),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "4000000000000000".to_string());

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "222222222222222".to_string());

//     let maximum_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: test_token2.issuer.clone(),
//                     currency: test_token2.currency.clone(),
//                     // Sending 1111111111111111 should truncate the amount to 1000000000000000 and should fail because bridge is already holding maximum
//                     amount: Uint128::new(1111111111111111),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(maximum_amount_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     // Test sending XRP
//     let denom = query_xrpl_tokens
//         .tokens
//         .iter()
//         .find(|t| t.issuer == XRP_ISSUER.to_string() && t.currency == XRP_CURRENCY.to_string())
//         .unwrap()
//         .coreum_denom
//         .clone();

//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: XRP_ISSUER.to_string(),
//                     currency: XRP_CURRENCY.to_string(),
//                     // Sending more than 100000000000000000 should fail because maximum holding amount is 10000000000000000 (1 less zero)
//                     amount: Uint128::new(100000000000000000),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: XRP_ISSUER.to_string(),
//                 currency: XRP_CURRENCY.to_string(),
//                 // There should never be truncation because we allow full precision for XRP initially
//                 amount: Uint128::one(),
//                 recipient: Addr::unchecked(receiver),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             Addr::unchecked(receiver),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "1".to_string());

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToOraiTransfer {
//                 tx_hash: generate_hash(),
//                 issuer: XRP_ISSUER.to_string(),
//                 currency: XRP_CURRENCY.to_string(),
//                 // This should work because we are sending the rest to reach the maximum amount
//                 amount: Uint128::new(9999999999999999),
//                 recipient: Addr::unchecked(receiver),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             Addr::unchecked(receiver),
//             denom: denom.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "10000000000000000".to_string());

//     let maximum_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToOraiTransfer {
//                     tx_hash: generate_hash(),
//                     issuer: XRP_ISSUER.to_string(),
//                     currency: XRP_CURRENCY.to_string(),
//                     // Sending 1 more token would surpass the maximum so should fail
//                     amount: Uint128::one(),
//                     recipient: Addr::unchecked(receiver),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(maximum_amount_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     // *** Test with Orai originated tokens ***

//     // Let's issue a few assets to the sender and registering them with different precisions and max sending amounts.

//     for i in 1..=3 {
//         let symbol = "TEST".to_string() + &i.to_string();
//         let subunit = "utest".to_string() + &i.to_string();
//         asset_ft
//             .issue(
//                 MsgIssue {
//                     issuer: "signer",
//                     symbol,
//                     subunit,
//                     precision: 6,
//                     initial_amount: "100000000000000".to_string(),
//                     description: "description".to_string(),
//                     features: vec![MINTING as i32],
//                     burn_rate: "0".to_string(),
//                     send_commission_rate: "0".to_string(),
//                     uri: "uri".to_string(),
//                     uri_hash: "uri_hash".to_string(),
//                 },
//                 Addr::unchecked(signer),
//             )
//             .unwrap();
//     }

//     let denom1 = format!("{}-{}", "utest1", "signer").to_lowercase();
//     let denom2 = format!("{}-{}", "utest2", "signer").to_lowercase();
//     let denom3 = format!("{}-{}", "utest3", "signer").to_lowercase();

//     let test_tokens = vec![
//         OraiToken {
//             denom: denom1.clone(),
//             decimals: 6,
//             sending_precision: 6,
//             max_holding_amount: Uint128::new(3),
//             bridging_fee: Uint128::zero(),
//         },
//         OraiToken {
//             denom: denom2.clone(),
//             decimals: 6,
//             sending_precision: 0,
//             max_holding_amount: Uint128::new(3990000),
//             bridging_fee: Uint128::zero(),
//         },
//         OraiToken {
//             denom: denom3.clone(),
//             decimals: 6,
//             sending_precision: -6,
//             max_holding_amount: Uint128::new(2000000000000),
//             bridging_fee: Uint128::zero(),
//         },
//     ];

//     // Register the tokens

//     for token in test_tokens.clone() {
//         app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RegisterOraiToken {
//                 denom: token.denom,
//                 decimals: token.decimals,
//                 sending_precision: token.sending_precision,
//                 max_holding_amount: token.max_holding_amount,
//                 bridging_fee: token.bridging_fee,
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap();
//     }

//     let query_coreum_tokens = wasm
//         .query::<QueryMsg, OraiTokensResponse>(
//             contract_addr.clone(),
//             &QueryMsg::OraiTokens {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();
//     assert_eq!(query_coreum_tokens.tokens.len(), 3);
//     assert_eq!(query_coreum_tokens.tokens[0].denom, test_tokens[0].denom);
//     assert_eq!(query_coreum_tokens.tokens[1].denom, test_tokens[1].denom);
//     assert_eq!(query_coreum_tokens.tokens[2].denom, test_tokens[2].denom);

//     // Test sending token 1 with high precision

//     // Sending 2 would work as it hasn't reached the maximum holding amount yet
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: generate_xrpl_address(),
//             deliver_amount: None,
//         },
//         &coins(2, denom1.clone()),
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Sending 1 more will hit max amount but will not fail
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: generate_xrpl_address(),
//             deliver_amount: None,
//         },
//         &coins(1, denom1.clone()),
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Trying to send 1 again would fail because we go over max bridge amount
//     let maximum_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: generate_xrpl_address(),
//                 deliver_amount: None,
//             },
//             &coins(1, denom1.clone()),
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(maximum_amount_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom1.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "3".to_string());

//     // Test sending token 2 with medium precision

//     // Sending under sending precision would return error because it will be truncated to 0.
//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: generate_xrpl_address(),
//                 deliver_amount: None,
//             },
//             &coins(100000, denom2.clone()),
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     // Sending 3990000 would work as it is the maximum bridgable amount
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: generate_xrpl_address(),
//             deliver_amount: None,
//         },
//         &coins(3990000, denom2.clone()),
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Sending 100000 will fail because truncating will truncate to 0.
//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: generate_xrpl_address(),
//                 deliver_amount: None,
//             },
//             &coins(100000, denom2.clone()),
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     // Trying to send 1000000 would fail because we go over max bridge amount
//     let maximum_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: generate_xrpl_address(),
//                 deliver_amount: None,
//             },
//             &coins(1000000, denom2.clone()),
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(maximum_amount_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom2.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "3990000".to_string());

//     // Test sending token 3 with low precision

//     // Sending 2000000000000 would work as it is the maximum bridgable amount
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: generate_xrpl_address(),
//             deliver_amount: None,
//         },
//         &coins(2000000000000, denom3.clone()),
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Sending 200000000000 (1 less zero) will fail because truncating will truncate to 0.
//     let precision_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: generate_xrpl_address(),
//                 deliver_amount: None,
//             },
//             &coins(200000000000, denom3.clone()),
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(precision_error.to_string().contains(
//         ContractError::AmountSentIsZeroAfterTruncation {}
//             .to_string()
//             .as_str()
//     ));

//     // Trying to send 1000000000000 would fail because we go over max bridge amount
//     let maximum_amount_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SendToXRPL {
//                 recipient: generate_xrpl_address(),
//                 deliver_amount: None,
//             },
//             &coins(1000000000000, denom3.clone()),
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(maximum_amount_error.to_string().contains(
//         ContractError::MaximumBridgedAmountReached {}
//             .to_string()
//             .as_str()
//     ));

//     let request_balance = asset_ft
//         .query_balance(&QueryBalanceRequest {
//             account: contract_addr.clone(),
//             denom: denom3.clone(),
//         })
//         .unwrap();

//     assert_eq!(request_balance.balance, "2000000000000".to_string());
// }
