use crate::contract::XRP_ISSUER;
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{PendingOperationsResponse, XRPLTokensResponse};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::token::full_denom;
use crate::{
    contract::XRP_CURRENCY,
    msg::{CosmosTokensResponse, ExecuteMsg, InstantiateMsg, QueryMsg},
    relayer::Relayer,
    state::TokenState,
};
use cosmwasm_std::{coins, Addr, Uint128};
use cosmwasm_testing_util::{MockAppExtensions, MockTokenExtensions};
use cw20::Cw20Coin;

#[test]
fn token_update() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let accounts_number = accounts.len();
    let signer = &accounts[accounts_number - 1];
    let xrpl_addresses: Vec<String> = (0..2).map(|_| generate_xrpl_address()).collect();
    let xrpl_pub_keys: Vec<String> = (0..2).map(|_| generate_xrpl_pub_key()).collect();

    let mut relayer_accounts = vec![];
    let mut relayers = vec![];

    for i in 0..accounts_number - 1 {
        relayer_accounts.push(accounts[i].to_string());
        relayers.push(Relayer {
            cosmos_address: Addr::unchecked(&accounts[i]),
            xrpl_address: xrpl_addresses[i as usize].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i as usize].to_string(),
        });
    }

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // Test with 1 relayer and 1 evidence threshold first
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayers[0].clone(), relayers[1].clone()],
                evidence_threshold: 2,
                used_ticket_sequence_threshold: 4,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();

    // Recover enough tickets for testing
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(5),
        },
        &[],
    )
    .unwrap();

    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: Some(1),
                    ticket_sequence: None,
                    transaction_result: TransactionResult::Accepted,
                    operation_result: Some(OperationResult::TicketsAllocation {
                        tickets: Some((1..6).collect()),
                    }),
                },
            },
            &[],
        )
        .unwrap();
    }

    // Register one XRPL token and one Cosmos token
    let issuer = generate_xrpl_address();
    let currency = "USD".to_string();

    let subunit = "utest".to_string();
    let initial_amount = Uint128::from(100000000u128);
    let decimals = 6;
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CreateCosmosToken {
            subdenom: subunit.to_uppercase(),
            initial_balances: vec![Cw20Coin {
                address: signer.to_string(),
                amount: initial_amount,
            }],
        },
        &coins(10_000_000u128, FEE_DENOM),
    )
    .unwrap();

    let denom = full_denom(&token_factory_addr, &subunit.to_uppercase());

    let sending_precision = 15;
    let max_holding_amount = Uint128::new(1000000000);
    let bridging_fee = Uint128::zero();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            sending_precision,
            max_holding_amount,
            bridging_fee,
        },
        &coins(10_000_000u128, FEE_DENOM),
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

    let xrpl_token_denom = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == issuer && t.currency == currency)
        .unwrap()
        .cosmos_denom
        .clone();

    // Updating XRP token to an invalid sending precision (more than decimals, 6) should fail
    let update_precision_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateXRPLToken {
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                state: None,
                sending_precision: Some(7),
                bridging_fee: None,
                max_holding_amount: None,
            },
            &[],
        )
        .unwrap_err();

    assert!(update_precision_error.root_cause().to_string().contains(
        ContractError::InvalidSendingPrecision {}
            .to_string()
            .as_str()
    ));

    // Updating XRP token to a valid sending precision (less than decimals, 6) should succeed
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: XRP_ISSUER.to_string(),
            currency: XRP_CURRENCY.to_string(),
            state: None,
            sending_precision: Some(5),
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // If we try to update the status of a token that is in processing state, it should fail
    let update_status_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateXRPLToken {
                issuer: issuer.clone(),
                currency: currency.clone(),
                state: Some(TokenState::Disabled),
                sending_precision: None,
                bridging_fee: None,
                max_holding_amount: None,
            },
            &[],
        )
        .unwrap_err();

    assert!(update_status_error
        .root_cause()
        .to_string()
        .contains(ContractError::TokenStateIsImmutable {}.to_string().as_str()));

    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: None,
                    ticket_sequence: Some(1),
                    transaction_result: TransactionResult::Accepted,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    // We will try to send one evidence with the token enabled and the other one with the token disabled, which should fail.
    let tx_hash = generate_hash();
    // First evidence should succeed
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::one(),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // Disable the token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: Some(TokenState::Disabled),
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // If we send second evidence it should fail because token is disabled
    let disabled_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[1]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::one(),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(disabled_error
        .root_cause()
        .to_string()
        .contains(ContractError::TokenNotEnabled {}.to_string().as_str()));

    // If we try to change the status to something that is not disabled or enabled it should fail
    let update_status_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateXRPLToken {
                issuer: issuer.clone(),
                currency: currency.clone(),
                state: Some(TokenState::Inactive),
                sending_precision: None,
                bridging_fee: None,
                max_holding_amount: None,
            },
            &[],
        )
        .unwrap_err();

    assert!(update_status_error.root_cause().to_string().contains(
        ContractError::InvalidTargetTokenState {}
            .to_string()
            .as_str()
    ));

    // If we try to change the status back to enabled and send the evidence, the balance should be sent to the receiver.
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: Some(TokenState::Enabled),
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::one(),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(signer), xrpl_token_denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "1".to_string());

    // If we disable again and we try to send the token back it will fail
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: Some(TokenState::Disabled),
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    let send_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1, xrpl_token_denom.clone()),
        )
        .unwrap_err();

    assert!(send_error
        .root_cause()
        .to_string()
        .contains(ContractError::TokenNotEnabled {}.to_string().as_str()));

    let sending_precision = 6;
    // Register the Cosmos Token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: denom.clone(),
            decimals,
            sending_precision,
            max_holding_amount,
            bridging_fee,
        },
        &[],
    )
    .unwrap();

    // If we try to change the status to something that is not disabled or enabled it should fail
    let update_status_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateCosmosToken {
                denom: denom.clone(),
                state: Some(TokenState::Processing),
                sending_precision: None,
                bridging_fee: None,
                max_holding_amount: None,
            },
            &[],
        )
        .unwrap_err();

    assert!(update_status_error.root_cause().to_string().contains(
        ContractError::InvalidTargetTokenState {}
            .to_string()
            .as_str()
    ));

    // Disable the Cosmos Token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateCosmosToken {
            denom: denom.clone(),
            state: Some(TokenState::Disabled),
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // If we try to send now it will fail because the token is disabled
    let send_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1, denom.clone()),
        )
        .unwrap_err();

    assert!(send_error
        .root_cause()
        .to_string()
        .contains(ContractError::TokenNotEnabled {}.to_string().as_str()));

    // Enable it again and modify the sending precision
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateCosmosToken {
            denom: denom.clone(),
            state: Some(TokenState::Enabled),
            sending_precision: Some(5),
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // Get the token information
    let query_cosmos_tokens: CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_cosmos_tokens.tokens[0].sending_precision, 5);

    // If we try to update to an invalid sending precision it should fail
    let update_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateCosmosToken {
                denom: denom.clone(),
                state: None,
                sending_precision: Some(7),
                bridging_fee: None,
                max_holding_amount: None,
            },
            &[],
        )
        .unwrap_err();

    assert!(update_error.root_cause().to_string().contains(
        ContractError::InvalidSendingPrecision {}
            .to_string()
            .as_str()
    ));

    // We will send 1 token and then modify the sending precision which should not allow the token to be sent with second evidence

    // Enable the token again (it was disabled)
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: Some(TokenState::Enabled),
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    let tx_hash = generate_hash();
    // First evidence should succeed
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::one(),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // Let's update the sending precision from 15 to 14
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: Some(14),
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    let evidence_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[1]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::one(),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(evidence_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // If we put it back to 15 and send, it should go through
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: Some(15),
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::one(),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // Let's send a bigger amount and check that it is truncated correctly after updating the sending precision
    let tx_hash = generate_hash();

    let previous_balance = app
        .query_balance(Addr::unchecked(signer), xrpl_token_denom.clone())
        .unwrap();
    let amount_to_send = 100001; // This should truncate 1 after updating sending precision and send 100000

    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(amount_to_send),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // Let's update the sending precision from 15 to 10
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: Some(10),
            bridging_fee: None,
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(amount_to_send),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    let new_balance = app
        .query_balance(Addr::unchecked(signer), xrpl_token_denom.clone())
        .unwrap();

    assert_eq!(
        new_balance.u128(),
        previous_balance
            .u128()
            .checked_add(amount_to_send)
            .unwrap()
            .checked_sub(1) // Truncated amount after updating sending precision
            .unwrap()
    );

    // Updating bridging fee for Cosmos Token should work
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateCosmosToken {
            denom: denom.clone(),
            state: None,
            sending_precision: None,
            bridging_fee: Some(Uint128::new(1000)),
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // Get the token information
    let query_cosmos_tokens: CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(
        query_cosmos_tokens.tokens[0].bridging_fee,
        Uint128::new(1000)
    );

    // Let's send an XRPL token evidence, modify the bridging fee, check that it's updated, and send the next evidence to see that bridging fee is applied correctly
    let amount_to_send = 1000000;

    let tx_hash = generate_hash();
    // First evidence should succeed
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(amount_to_send),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // Let's update the bridging fee from 0 to 10000000
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: None,
            bridging_fee: Some(Uint128::new(10000000)),
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // If we try to send the second evidence it should fail because we can't cover new updated bridging fee
    let bridging_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[1]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::new(amount_to_send),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(bridging_error.root_cause().to_string().contains(
        ContractError::CannotCoverBridgingFees {}
            .to_string()
            .as_str()
    ));

    // Let's update the bridging fee from 0 to 100000
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: None,
            bridging_fee: Some(Uint128::new(1000000)),
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // If we try to send the second evidence it should fail because amount is 0 after applying bridging fees
    let bridging_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[1]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::new(amount_to_send),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(bridging_error.root_cause().to_string().contains(
        ContractError::AmountSentIsZeroAfterTruncation {}
            .to_string()
            .as_str()
    ));

    // Let's update the bridging fee from 0 to 1000
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: None,
            bridging_fee: Some(Uint128::new(1000)),
            max_holding_amount: None,
        },
        &[],
    )
    .unwrap();

    // Sending evidence should succeed
    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(amount_to_send),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    let previous_balance = new_balance;
    let new_balance = app
        .query_balance(Addr::unchecked(signer), xrpl_token_denom.clone())
        .unwrap();

    assert_eq!(
        new_balance.u128(),
        previous_balance
            .u128()
            .checked_add(amount_to_send) // 1000000 - 1000 (bridging fee) = 999000
            .unwrap()
            .checked_sub(1000) // bridging fee
            .unwrap()
            .checked_sub(99000) // Truncated amount after applying bridging fees (sending precision is 10) = 999000 -> 900000
            .unwrap()
    );

    // Let's bridge some tokens from Cosmos to XRPL to have some amount in the bridge
    let current_max_amount = 10000;
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(current_max_amount, denom.clone()),
    )
    .unwrap();

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

    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: None,
                    ticket_sequence: Some(
                        query_pending_operations.operations[0]
                            .ticket_sequence
                            .unwrap(),
                    ),
                    transaction_result: TransactionResult::Accepted,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), current_max_amount.to_string());

    // Updating max holding amount for Cosmos Token should work with less than current holding amount should not work
    let error_update = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateCosmosToken {
                denom: denom.clone(),
                state: None,
                sending_precision: None,
                bridging_fee: None,
                max_holding_amount: Some(Uint128::new(current_max_amount - 1)),
            },
            &[],
        )
        .unwrap_err();

    assert!(error_update.root_cause().to_string().contains(
        ContractError::InvalidTargetMaxHoldingAmount {}
            .to_string()
            .as_str()
    ));

    // Updating max holding amount with more than current holding amount should work
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateCosmosToken {
            denom: denom.clone(),
            state: None,
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: Some(Uint128::new(current_max_amount + 1)),
        },
        &[],
    )
    .unwrap();

    let query_cosmos_tokens: CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(
        query_cosmos_tokens.tokens[0].max_holding_amount,
        Uint128::new(current_max_amount + 1)
    );

    // Let's send an XRPL token evidence, modify the max_holding_amount, check that it's updated, and send the next evidence to see
    // that max_holding_amount checks are applied correctly

    // Get current bridged amount
    let total_supply = app.query_supply(&xrpl_token_denom).unwrap();
    let current_bridged_amount = total_supply.amount;

    // Let's update the max holding amount with current bridged amount - 1 (it should fail)
    let update_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateXRPLToken {
                issuer: issuer.clone(),
                currency: currency.clone(),
                state: None,
                sending_precision: None,
                bridging_fee: None,
                max_holding_amount: Some(Uint128::new(current_bridged_amount.u128() - 1)),
            },
            &[],
        )
        .unwrap_err();

    assert!(update_error.root_cause().to_string().contains(
        ContractError::InvalidTargetMaxHoldingAmount {}
            .to_string()
            .as_str()
    ));

    // Let's send the first XRPL transfer evidence
    let amount_to_send = 1001000;

    let tx_hash = generate_hash();
    // First evidence should succeed
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(amount_to_send),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // Let's update the max holding amount with current bridged amount + amount to send - 1 (it should fail in next evidence send because it won't be enough)
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: Some(Uint128::new(
                current_bridged_amount.u128() + amount_to_send - 1,
            )),
        },
        &[],
    )
    .unwrap();

    // If we try to send the second evidence it should fail because we can't go over max holding amount
    let bridging_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[1]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::new(amount_to_send),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(bridging_error.root_cause().to_string().contains(
        ContractError::MaximumBridgedAmountReached {}
            .to_string()
            .as_str()
    ));

    // Get previous balance of user to compare later
    let previous_balance_user = app
        .query_balance(Addr::unchecked(signer), xrpl_token_denom.clone())
        .unwrap();

    // Let's update the max holding amount with current bridged amount + amount to send (second evidence should go through)
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            state: None,
            sending_precision: None,
            bridging_fee: None,
            max_holding_amount: Some(Uint128::new(current_bridged_amount.u128() + amount_to_send)),
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(amount_to_send),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    let new_balance_user = app
        .query_balance(Addr::unchecked(signer), xrpl_token_denom.clone())
        .unwrap();

    // Check balance has been sent to user
    assert_eq!(
        new_balance_user.u128(),
        previous_balance_user
            .u128()
            .checked_add(amount_to_send)
            .unwrap()
            .checked_sub(1000) // bridging fee
            .unwrap()
    );
}

#[test]
fn test_burning_rate_and_commission_fee_cosmos_tokens() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let relayer_account = &accounts[1];
    let sender = &accounts[2];
    let relayer = Relayer {
        cosmos_address: Addr::unchecked(relayer_account),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let xrpl_receiver_address = generate_xrpl_address();
    let bridge_xrpl_address = generate_xrpl_address();

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // Test with 1 relayer and 1 evidence threshold first
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 9,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: bridge_xrpl_address.clone(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();

    // Add enough tickets for all our test operations

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(10),
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..11).collect()),
                }),
            },
        },
        &[],
    )
    .unwrap();

    // Let's issue a token with burning and commission fees and make sure it works out of the box

    let subunit = "utest".to_string();
    let decimals = 6;
    let initial_amount = Uint128::new(10000000000);

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CreateCosmosToken {
            subdenom: subunit.to_uppercase(),
            initial_balances: vec![Cw20Coin {
                address: signer.to_string(),
                amount: initial_amount,
            }],
        },
        &coins(10_000_000u128, FEE_DENOM),
    )
    .unwrap();

    let denom = full_denom(&token_factory_addr, &subunit.to_uppercase());

    // Let's transfer some tokens to a sender from the issuer so that we can check both rates being applied
    app.send_coins(
        Addr::unchecked(signer),
        Addr::unchecked(sender),
        &coins(100000000, denom.clone()),
    )
    .unwrap();

    // Check the balance
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "100000000".to_string());

    // Let's try to bridge some tokens and back and check that everything works correctly
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: denom.clone(),
            decimals,
            sending_precision: 6,
            max_holding_amount: Uint128::new(1000000000),
            bridging_fee: Uint128::zero(),
        },
        &[],
    )
    .unwrap();

    println!(
        "balance {}",
        app.query_balance(Addr::unchecked(sender), denom.clone())
            .unwrap()
    );

    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(100, denom.clone()),
    )
    .unwrap();

    // This should have burned an extra 0 and charged 0 tokens as commission fee to the sender. Let's check just in case
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "99999900".to_string());

    // Let's check that only 100 tokens are in the contract
    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "100".to_string());

    // Let's confirm the briding XRPL and bridge the entire amount back to Cosmos
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
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: query_pending_operations.operations[0].account_sequence,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    // Get the token information
    let query_cosmos_tokens: CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let cosmos_originated_token = query_cosmos_tokens
        .tokens
        .iter()
        .find(|t| t.denom == denom)
        .unwrap();

    let amount_to_send_back = Uint128::new(100_000_000_000); // 100 utokens on Cosmos are represented as 1e11 on XRPL
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: bridge_xrpl_address.clone(),
                currency: cosmos_originated_token.xrpl_currency.clone(),
                amount: amount_to_send_back.clone(),
                recipient: Addr::unchecked(Addr::unchecked(sender)),
            },
        },
        &[],
    )
    .unwrap();

    // Check that the sender received the correct amount (100 tokens) and contract doesn't have anything left
    // This way we confirm that contract is not affected by commission fees and burn rate
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "100000000".to_string());

    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "0".to_string());
}
