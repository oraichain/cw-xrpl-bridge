// #[test]
// fn ticket_recovery() {
//     let app = OraiTestApp::new();
//     let accounts_number = 3;
//     let accounts = app
//         .init_accounts(&coins(100_000_000_000, FEE_DENOM), accounts_number)
//         .unwrap();

//     let signer = accounts.get((accounts_number - 1) as usize).unwrap();
//     let xrpl_addresses = vec![generate_xrpl_address(), generate_xrpl_address()];

//     let xrpl_pub_keys = vec![generate_xrpl_pub_key(), generate_xrpl_pub_key()];

//     let mut relayer_accounts = vec![];
//     let mut relayers = vec![];

//     for i in 0..accounts_number - 1 {
//         relayer_accounts.push(accounts.get(i as usize).unwrap());
//         relayers.push(Relayer {
//             cosmos_address: Addr::unchecked(accounts.get(i as usize).unwrap().address()),
//             xrpl_address: xrpl_addresses[i as usize].to_string(),
//             xrpl_pub_key: xrpl_pub_keys[i as usize].to_string(),
//         });
//     }

//

//     let xrpl_base_fee = 10;

//     let contract_addr = store_and_instantiate(
//         &wasm,
//         Addr::unchecked(signer),
//         Addr::unchecked(signer),
//         vec![relayers[0].clone(), relayers[1].clone()],
//         2,
//         4,
//         Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//         query_issue_fee(&asset_ft),
//         generate_xrpl_address(),
//         xrpl_base_fee,
//     );

//     // Querying current pending operations and available tickets should return empty results.
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let query_available_tickets = wasm
//         .query::<QueryMsg, AvailableTicketsResponse>(contract_addr.clone(), &QueryMsg::AvailableTickets {})
//         .unwrap();

//     assert!(query_pending_operations.operations.is_empty());
//     assert!(query_available_tickets.tickets.is_empty());

//     let account_sequence = 1;
//     // Trying to recover tickets with the value less than used_ticket_sequence_threshold
//     let recover_ticket_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RecoverTickets {
//                 account_sequence,
//                 number_of_tickets: Some(1),
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(recover_ticket_error.root_cause().to_string().contains(
//         ContractError::InvalidTicketSequenceToAllocate {}
//             .to_string()
//             .as_str()
//     ));

//     // Trying to recover more than max tickets will fail
//     let recover_ticket_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RecoverTickets {
//                 account_sequence,
//                 number_of_tickets: Some(300),
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(recover_ticket_error.root_cause().to_string().contains(
//         ContractError::InvalidTicketSequenceToAllocate {}
//             .to_string()
//             .as_str()
//     ));

//     // Trying to recover more than max tickets will fail
//     let recover_ticket_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RecoverTickets {
//                 account_sequence,
//                 number_of_tickets: Some(300),
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(recover_ticket_error.root_cause().to_string().contains(
//         ContractError::InvalidTicketSequenceToAllocate {}
//             .to_string()
//             .as_str()
//     ));

//     // Check that we can recover tickets and provide signatures for this operation with the bridge halted
//     app.execute(contract_addr.clone(), &ExecuteMsg::HaltBridge {}, &[], Addr::unchecked(signer))
//         .unwrap();

//     // Owner will send a recover tickets operation which will set the pending ticket update flag to true
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence,
//             number_of_tickets: Some(5),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Try to send another one will fail because there is a pending update operation that hasn't been processed
//     let recover_ticket_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RecoverTickets {
//                 account_sequence,
//                 number_of_tickets: Some(5),
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(recover_ticket_error
//         .to_string()
//         .contains(ContractError::PendingTicketUpdate {}.to_string().as_str()));

//     // Querying the current pending operations should return 1
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(
//         query_pending_operations.operations,
//         [Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: None,
//             account_sequence: Some(account_sequence),
//             signatures: vec![], // No signatures yet
//             operation_type: OperationType::AllocateTickets { number: 5 },
//             xrpl_base_fee
//         }]
//     );

//     let tx_hash = generate_hash();
//     let tickets = vec![1, 2, 3, 4, 5];
//     let correct_signature_example = "3045022100DFA01DA5D6C9877F9DAA59A06032247F3D7ED6444EAD5C90A3AC33CCB7F19B3F02204D8D50E4D085BB1BC9DFB8281B8F35BDAEB7C74AE4B825F8CAE1217CFBDF4EA1".to_string();

//     // Trying to relay the operation with a different sequence number than the one in pending operation should fail.
//     let relayer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(tx_hash.clone()),
//                     account_sequence: Some(account_sequence + 1),
//                     ticket_sequence: None,
//                     transaction_result: TransactionResult::Rejected,
//                     operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
//                 },
//             },
//             &[],
//             Addr::unchecked(&relayer_accounts[0])
//         )
//         .unwrap_err();

//     assert!(relayer_error.root_cause().to_string().contains(
//         ContractError::PendingOperationNotFound {}
//             .to_string()
//             .as_str()
//     ));

//     // Providing an invalid signature for the operation should error
//     let signature_error = app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveSignature {
//                 operation_id: account_sequence,
//                 operation_version: 1,
//                 signature: "3045022100DFA01DA5D6C9877F9DAA59A06032247F3D7ED6444EAD5C90A3AC33CCB7F19B3F02204D8D50E4D085BB1BC9DFB8281B8F35BDAEB7C74AE4B825F8CAE1217CFBDF4EA13045022100DFA01DA5D6C9877F9DAA59A06032247F3D7ED6444EAD5C90A3AC33CCB7F19B3F02204D8D50E4D085BB1BC9DFB8281B8F35BDAEB7C74AE4B825F8CAE1217CFBDF4EA1".to_string(),
//             },
//             &[],
//             Addr::unchecked(&relayer_accounts[0])
//         )
//         .unwrap_err();

//     assert!(signature_error.root_cause().to_string().contains(
//         ContractError::InvalidSignatureLength {}
//             .to_string()
//             .as_str()
//     ));

//     // Provide signatures for the operation for each relayer
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveSignature {
//             operation_id: account_sequence,
//             operation_version: 1,
//             signature: correct_signature_example.clone(),
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[0])
//     )
//     .unwrap();

//     // Provide the signature again for the operation will fail
//     let signature_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveSignature {
//                 operation_id: account_sequence,
//                 operation_version: 1,
//                 signature: correct_signature_example.clone(),
//             },
//             &[],
//             Addr::unchecked(&relayer_accounts[0])
//         )
//         .unwrap_err();

//     assert!(signature_error.root_cause().to_string().contains(
//         ContractError::SignatureAlreadyProvided {}
//             .to_string()
//             .as_str()
//     ));

//     // Provide a signature for an operation that is not pending should fail
//     let signature_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveSignature {
//                 operation_id: account_sequence + 1,
//                 operation_version: 1,
//                 signature: correct_signature_example.clone(),
//             },
//             &[],
//             Addr::unchecked(&relayer_accounts[0])
//         )
//         .unwrap_err();

//     assert!(signature_error.root_cause().to_string().contains(
//         ContractError::PendingOperationNotFound {}
//             .to_string()
//             .as_str()
//     ));

//     // Provide a signature for an operation with a different version should fail
//     let signature_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveSignature {
//                 operation_id: account_sequence,
//                 operation_version: 2,
//                 signature: correct_signature_example.clone(),
//             },
//             &[],
//             Addr::unchecked(&relayer_accounts[0])
//         )
//         .unwrap_err();

//     assert!(signature_error.root_cause().to_string().contains(
//         ContractError::OperationVersionMismatch {}
//             .to_string()
//             .as_str()
//     ));

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveSignature {
//             operation_id: account_sequence,
//             operation_version: 1,
//             signature: correct_signature_example.clone(),
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[1])
//     )
//     .unwrap();

//     // Verify that we have both signatures in the operation
//     let query_pending_operation = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operation.operations.len(), 1);
//     assert_eq!(
//         query_pending_operation.operations[0].signatures,
//         vec![
//             Signature {
//                 signature: correct_signature_example.clone(),
//                 relayer_cosmos_address: Addr::unchecked(relayers[0].cosmos_address.clone()),
//             },
//             Signature {
//                 signature: correct_signature_example.clone(),
//                 relayer_cosmos_address: Addr::unchecked(relayers[1].cosmos_address.clone()),
//             }
//         ]
//     );

//     // Relaying the rejected operation twice should remove it from pending operations but not allocate tickets
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(tx_hash.clone()),
//                 account_sequence: Some(account_sequence),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
//             },
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[0])
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(tx_hash.clone()),
//                 account_sequence: Some(account_sequence),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Rejected,
//                 operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
//             },
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[1])
//     )
//     .unwrap();

//     // Querying current pending operations and tickets should return empty results again
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let query_available_tickets = wasm
//         .query::<QueryMsg, AvailableTicketsResponse>(contract_addr.clone(), &QueryMsg::AvailableTickets {})
//         .unwrap();

//     assert_eq!(query_pending_operations.operations, vec![]);
//     assert_eq!(query_available_tickets.tickets, Vec::<u64>::new());

//     // Resume bridge
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ResumeBridge {},
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Let's do the same now but reporting an invalid transaction
//     let account_sequence = 2;
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence,
//             number_of_tickets: Some(5),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // We provide the signatures again
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveSignature {
//             operation_id: account_sequence,
//             operation_version: 1,
//             signature: correct_signature_example.clone(),
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[0])
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveSignature {
//             operation_id: account_sequence,
//             operation_version: 1,
//             signature: correct_signature_example.clone(),
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[1])
//     )
//     .unwrap();
//     // Trying to relay the operation with a same hash as previous rejected one should fail
//     let relayer_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(tx_hash.clone()),
//                     account_sequence: Some(account_sequence),
//                     ticket_sequence: None,
//                     transaction_result: TransactionResult::Accepted,
//                     operation_result: Some(OperationResult::TicketsAllocation {
//                         tickets: Some(tickets.clone()),
//                     }),
//                 },
//             },
//             &[],
//             Addr::unchecked(&relayer_accounts[0])
//         )
//         .unwrap_err();

//     assert!(relayer_error.root_cause().to_string().contains(
//         ContractError::OperationAlreadyExecuted {}
//             .to_string()
//             .as_str()
//     ));

//     // Relaying the operation twice as invalid should removed it from pending operations and not allocate tickets
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: None,
//                 account_sequence: Some(account_sequence),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Invalid,
//                 operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
//             },
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[0])
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: None,
//                 account_sequence: Some(account_sequence),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Invalid,
//                 operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
//             },
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[1])
//     )
//     .unwrap();

//     // Querying the current pending operations should return empty
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let query_available_tickets = wasm
//         .query::<QueryMsg, AvailableTicketsResponse>(contract_addr.clone(), &QueryMsg::AvailableTickets {})
//         .unwrap();

//     assert_eq!(query_pending_operations.operations, vec![]);
//     assert_eq!(query_available_tickets.tickets, Vec::<u64>::new());

//     // Let's do the same now but confirming the operation

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence,
//             number_of_tickets: Some(5),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let tx_hash = generate_hash();

//     // Relaying the accepted operation twice should remove it from pending operations and allocate tickets
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(tx_hash.clone()),
//                 account_sequence: Some(account_sequence),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: Some(OperationResult::TicketsAllocation {
//                     tickets: Some(tickets.clone()),
//                 }),
//             },
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[0])
//     )
//     .unwrap();

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: Some(tx_hash.clone()),
//                 account_sequence: Some(account_sequence),
//                 ticket_sequence: None,
//                 transaction_result: TransactionResult::Accepted,
//                 operation_result: Some(OperationResult::TicketsAllocation {
//                     tickets: Some(tickets.clone()),
//                 }),
//             },
//         },
//         &[],
//         Addr::unchecked(&relayer_accounts[1])
//     )
//     .unwrap();

//     // Querying the current pending operations should return empty
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let query_available_tickets = wasm
//         .query::<QueryMsg, AvailableTicketsResponse>(contract_addr.clone(), &QueryMsg::AvailableTickets {})
//         .unwrap();

//     assert_eq!(query_pending_operations.operations, vec![]);
//     assert_eq!(query_available_tickets.tickets, tickets.clone());
// }

// #[test]
// fn rejected_ticket_allocation_with_no_tickets_left() {
//     let app = OraiTestApp::new();
//     let signer = app
//         .init_account(&coins(100_000_000_000, FEE_DENOM))
//         .unwrap();

//

//     let relayer = Relayer {
//         cosmos_address: Addr::unchecked(signer),
//         xrpl_address: generate_xrpl_address(),
//         xrpl_pub_key: generate_xrpl_pub_key(),
//     };

//     let test_tokens = vec![
//         XRPLToken {
//             issuer: generate_xrpl_address(), // Valid issuer
//             currency: "USD".to_string(),     // Valid standard currency code
//             sending_precision: -15,
//             max_holding_amount: Uint128::new(100),
//             bridging_fee: Uint128::zero(),
//         },
//         XRPLToken {
//             issuer: generate_xrpl_address(), // Valid issuer
//             currency: "015841551A748AD2C1F76FF6ECB0CCCD00000000".to_string(), // Valid hexadecimal currency
//             sending_precision: 15,
//             max_holding_amount: Uint128::new(50000),
//             bridging_fee: Uint128::zero(),
//         },
//     ];
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

//     // We successfully recover 3 tickets
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

//     // We register and enable 2 tokens, which should trigger a second ticket allocation with the last available ticket.
//     for (index, token) in test_tokens.iter().enumerate() {
//         app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RegisterXRPLToken {
//                 issuer: token.issuer.clone(),
//                 currency: token.currency.clone(),
//                 sending_precision: token.sending_precision,
//                 max_holding_amount: token.max_holding_amount,
//                 bridging_fee: token.bridging_fee,
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap();

//         app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(generate_hash()),
//                     account_sequence: None,
//                     ticket_sequence: Some(u64::try_from(index).unwrap() + 1),
//                     transaction_result: TransactionResult::Accepted,
//                     operation_result: None,
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap();
//     }

//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let query_available_tickets = wasm
//         .query::<QueryMsg, AvailableTicketsResponse>(contract_addr.clone(), &QueryMsg::AvailableTickets {})
//         .unwrap();

//     assert_eq!(
//         query_pending_operations.operations,
//         [Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: Some(3),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::AllocateTickets { number: 2 },
//             xrpl_base_fee,
//         }]
//     );
//     assert_eq!(query_available_tickets.tickets, Vec::<u64>::new());

//     // If we reject this operation, it should trigger a new ticket allocation but since we have no tickets available, it should
//     // NOT fail (because otherwise contract will be stuck) but return an additional attribute warning that there are no available tickets left
//     // requiring a manual ticket recovery in the future.
//     let result = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(generate_hash()),
//                     account_sequence: None,
//                     ticket_sequence: Some(3),
//                     transaction_result: TransactionResult::Rejected,
//                     operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
//                 },
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
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

//     let query_available_tickets = wasm
//         .query::<QueryMsg, AvailableTicketsResponse>(contract_addr.clone(), &QueryMsg::AvailableTickets {})
//         .unwrap();

//     assert!(query_pending_operations.operations.is_empty());
//     assert!(query_available_tickets.tickets.is_empty());
//     assert!(result.events.iter().any(|e| e.ty == "wasm"
//         && e.attributes
//             .iter()
//             .any(|a| a.key == "adding_ticket_allocation_operation_success"
//                 && a.value == false.to_string())));
// }

// #[test]
// fn ticket_return_invalid_transactions() {
//     let app = OraiTestApp::new();
//     let accounts_number = 3;
//     let accounts = app
//         .init_accounts(&coins(100_000_000_000, FEE_DENOM), accounts_number)
//         .unwrap();

//     let signer = accounts.get(0).unwrap();
//     let sender = accounts.get(1).unwrap();
//     let relayer_account = accounts.get(2).unwrap();
//     let relayer = Relayer {
//         cosmos_address: Addr::unchecked(relayer),
//         xrpl_address: generate_xrpl_address(),
//         xrpl_pub_key: generate_xrpl_pub_key(),
//     };

//     let xrpl_receiver_address = generate_xrpl_address();
//     let bridge_xrpl_address = generate_xrpl_address();

//

//     let contract_addr = store_and_instantiate(
//         &wasm,
//         signer,
//         Addr::unchecked(signer),
//         vec![relayer.clone()],
//         1,
//         5,
//         Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//         query_issue_fee(&asset_ft),
//         bridge_xrpl_address.clone(),
//         10,
//     );

//     // Add enough tickets to test that ticket is correctly returned

//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence: 1,
//             number_of_tickets: Some(6),
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
//                     tickets: Some((1..7).collect()),
//                 }),
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Let's issue a token and register it

//     let symbol = "TEST".to_string();
//     let subunit = "utest".to_string();
//     let decimals = 6;
//     let initial_amount = Uint128::new(100000000);
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
//         &ExecuteMsg::RegisterCosmosToken {
//             denom: denom.clone(),
//             decimals,
//             sending_precision: 6,
//             max_holding_amount: Uint128::new(10000000),
//             bridging_fee: Uint128::zero(),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // We are going to bridge a token and reject the operation
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(1, denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     // Get the current ticket used to compare later
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     let ticket_used_invalid_operation = query_pending_operations.operations[0]
//         .ticket_sequence
//         .unwrap();

//     // Send evidence of invalid operation, which should return the ticket to the ticket array
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLTransactionResult {
//                 tx_hash: None,
//                 account_sequence: query_pending_operations.operations[0].account_sequence,
//                 ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
//                 transaction_result: TransactionResult::Invalid,
//                 operation_result: None,
//             },
//         },
//         &[],
//         relayer_account,
//     )
//     .unwrap();

//     // Now let's try to send again and verify that the ticket is the same as before (it was given back)
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SendToXRPL {
//             recipient: xrpl_receiver_address.clone(),
//             deliver_amount: None,
//         },
//         &coins(1, denom.clone()),
//         &sender,
//     )
//     .unwrap();

//     // Get the current ticket used to compare later
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(
//         ticket_used_invalid_operation,
//         query_pending_operations.operations[0]
//             .ticket_sequence
//             .unwrap()
//     );
// }
