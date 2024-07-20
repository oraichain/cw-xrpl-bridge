// #[test]
// fn xrpl_token_registration_recovery() {
//     let app = CosmosTestApp::new();
//     let signer = app
//         .init_account(&coins(100_000_000_000, FEE_DENOM))
//         .unwrap();
//

//     let relayer = Relayer {
//         cosmos_address: Addr::unchecked(signer),
//         xrpl_address: generate_xrpl_address(),
//         xrpl_pub_key: generate_xrpl_pub_key(),
//     };

//     let token_issuer = generate_xrpl_address();
//     let token_currency = "BTC".to_string();
//     let token = XRPLToken {
//         issuer: token_issuer.clone(),
//         currency: token_currency.clone(),
//         sending_precision: -15,
//         max_holding_amount: Uint128::new(100),
//         bridging_fee: Uint128::zero(),
//     };
//     let xrpl_base_fee = 10;

//     let contract_addr = store_and_instantiate(
//         &wasm,
//         Addr::unchecked(signer),
//         Addr::unchecked(signer),
//         vec![relayer.clone()],
//         1,
//         2,
//         Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//         query_issue_fee(&asset_ft),
//         generate_xrpl_address(),
//         xrpl_base_fee,
//     );

//     // We successfully recover 3 tickets to perform operations
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence: 1,
//             number_of_tickets: Some(3),
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
//                     tickets: Some((1..4).collect()),
//                 }),
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // We perform the register token operation, which should put the token to Processing state and create the PendingOperation
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RegisterXRPLToken {
//             issuer: token.issuer.clone(),
//             currency: token.currency.clone(),
//             sending_precision: token.sending_precision,
//             max_holding_amount: token.max_holding_amount,
//             bridging_fee: token.bridging_fee,
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // If we try to recover a token that is not in Inactive state, it should fail.
//     let recover_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RecoverXRPLTokenRegistration {
//                 issuer: token.issuer.clone(),
//                 currency: token.currency.clone(),
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(recover_error
//         .to_string()
//         .contains(ContractError::XRPLTokenNotInactive {}.to_string().as_str()));

//     // If we try to recover a token that is not registered, it should fail
//     let recover_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RecoverXRPLTokenRegistration {
//                 issuer: token.issuer.clone(),
//                 currency: "NOT".to_string(),
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(recover_error
//         .to_string()
//         .contains(ContractError::TokenNotRegistered {}.to_string().as_str()));

//     // Let's fail the trust set operation to put the token to Inactive so that we can recover it

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

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(generate_hash()),
//                 account_sequence: None,
//                 ticket_sequence: Some(
//                     query_pending_operations.operations[0]
//                         .ticket_sequence
//                         .unwrap(),
//                 ),
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: None,
//             },
//         },
//         &[],
//         Addr::unchecked(signer),
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

//     assert!(query_pending_operations.operations.is_empty());

//     // We should be able to recover the token now
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverXRPLTokenRegistration {
//             issuer: token.issuer.clone(),
//             currency: token.currency.clone(),
//         },
//         &[],
//         Addr::unchecked(signer),
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
//             ticket_sequence: Some(
//                 query_pending_operations.operations[0]
//                     .ticket_sequence
//                     .unwrap()
//             ),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::TrustSet {
//                 issuer: token_issuer,
//                 currency: token_currency,
//                 trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//             },
//             xrpl_base_fee,
//         }
//     );
// }
