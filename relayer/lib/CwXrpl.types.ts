export type Addr = string;
export type Uint128 = string;
export interface InstantiateMsg {
  bridge_xrpl_address: string;
  evidence_threshold: number;
  issue_token: boolean;
  owner: Addr;
  relayers: Relayer[];
  token_factory_addr: Addr;
  trust_set_limit_amount: Uint128;
  used_ticket_sequence_threshold: number;
  xrpl_base_fee: number;
}
export interface Relayer {
  cosmos_address: Addr;
  xrpl_address: string;
  xrpl_pub_key: string;
}
export type ExecuteMsg = {
  create_cosmos_token: {
    decimals: number;
    description?: string | null;
    initial_balances: Cw20Coin[];
    name?: string | null;
    subdenom: string;
    symbol?: string | null;
  };
} | {
  mint_cosmos_token: {
    denom: string;
    initial_balances: Cw20Coin[];
  };
} | {
  register_cosmos_token: {
    bridging_fee: Uint128;
    decimals: number;
    denom: string;
    max_holding_amount: Uint128;
    sending_precision: number;
  };
} | {
  register_xrpl_token: {
    bridging_fee: Uint128;
    currency: string;
    issuer: string;
    max_holding_amount: Uint128;
    sending_precision: number;
  };
} | {
  recover_tickets: {
    account_sequence: number;
    number_of_tickets?: number | null;
  };
} | {
  recover_xrpl_token_registration: {
    currency: string;
    issuer: string;
  };
} | {
  save_signature: {
    operation_id: number;
    operation_version: number;
    signature: string;
  };
} | {
  save_evidence: {
    evidence: Evidence;
  };
} | {
  send_to_xrpl: {
    deliver_amount?: Uint128 | null;
    recipient: string;
  };
} | {
  update_xrpl_token: {
    bridging_fee?: Uint128 | null;
    currency: string;
    issuer: string;
    max_holding_amount?: Uint128 | null;
    sending_precision?: number | null;
    state?: TokenState | null;
  };
} | {
  update_cosmos_token: {
    bridging_fee?: Uint128 | null;
    denom: string;
    max_holding_amount?: Uint128 | null;
    sending_precision?: number | null;
    state?: TokenState | null;
  };
} | {
  update_xrpl_base_fee: {
    xrpl_base_fee: number;
  };
} | {
  claim_refund: {
    pending_refund_id: string;
  };
} | {
  claim_relayer_fees: {
    amounts: Coin[];
  };
} | {
  halt_bridge: {};
} | {
  resume_bridge: {};
} | {
  rotate_keys: {
    new_evidence_threshold: number;
    new_relayers: Relayer[];
  };
} | {
  update_prohibited_xrpl_addresses: {
    prohibited_xrpl_addresses: string[];
  };
} | {
  cancel_pending_operation: {
    operation_id: number;
  };
} | {
  update_ownership: Action;
};
export type Evidence = {
  xrpl_to_cosmos_transfer: {
    amount: Uint128;
    currency: string;
    issuer: string;
    recipient: Addr;
    tx_hash: string;
  };
} | {
  xrpl_transaction_result: {
    account_sequence?: number | null;
    operation_result?: OperationResult | null;
    ticket_sequence?: number | null;
    transaction_result: TransactionResult;
    tx_hash?: string | null;
  };
};
export type OperationResult = {
  tickets_allocation: {
    tickets?: number[] | null;
  };
};
export type TransactionResult = "accepted" | "rejected" | "invalid";
export type TokenState = "enabled" | "disabled" | "processing" | "inactive";
export type Action = {
  transfer_ownership: {
    expiry?: Expiration | null;
    new_owner: string;
  };
} | "accept_ownership" | "renounce_ownership";
export type Expiration = {
  at_height: number;
} | {
  at_time: Timestamp;
} | {
  never: {};
};
export type Timestamp = Uint64;
export type Uint64 = string;
export interface Cw20Coin {
  address: string;
  amount: Uint128;
}
export interface Coin {
  amount: Uint128;
  denom: string;
}
export type QueryMsg = {
  config: {};
} | {
  xrpl_tokens: {
    limit?: number | null;
    start_after_key?: string | null;
  };
} | {
  cosmos_tokens: {
    limit?: number | null;
    start_after_key?: string | null;
  };
} | {
  pending_operations: {
    limit?: number | null;
    start_after_key?: number | null;
  };
} | {
  available_tickets: {};
} | {
  fees_collected: {
    relayer_address: Addr;
  };
} | {
  pending_refunds: {
    address: Addr;
    limit?: number | null;
    start_after_key?: [Addr, string] | null;
  };
} | {
  bridge_state: {};
} | {
  transaction_evidence: {
    hash: string;
  };
} | {
  transaction_evidences: {
    limit?: number | null;
    start_after_key?: string | null;
  };
} | {
  processed_tx: {
    hash: string;
  };
} | {
  processed_txs: {
    limit?: number | null;
    start_after_key?: string | null;
  };
} | {
  prohibited_xrpl_addresses: {};
} | {
  ownership: {};
};
export interface MigrateMsg {}
export interface AvailableTicketsResponse {
  tickets: number[];
}
export type BridgeState = "active" | "halted";
export interface BridgeStateResponse {
  state: BridgeState;
}
export interface Config {
  bridge_state: BridgeState;
  bridge_xrpl_address: string;
  evidence_threshold: number;
  relayers: Relayer[];
  token_factory_addr: Addr;
  trust_set_limit_amount: Uint128;
  used_ticket_sequence_threshold: number;
  xrpl_base_fee: number;
}
export interface CosmosTokensResponse {
  last_key?: string | null;
  tokens: CosmosToken[];
}
export interface CosmosToken {
  bridging_fee: Uint128;
  decimals: number;
  denom: string;
  max_holding_amount: Uint128;
  sending_precision: number;
  state: TokenState;
  xrpl_currency: string;
}
export interface FeesCollectedResponse {
  fees_collected: Coin[];
}
export interface OwnershipForString {
  owner?: string | null;
  pending_expiry?: Expiration | null;
  pending_owner?: string | null;
}
export type OperationType = {
  allocate_tickets: {
    number: number;
  };
} | {
  trust_set: {
    currency: string;
    issuer: string;
    trust_set_limit_amount: Uint128;
  };
} | {
  rotate_keys: {
    new_evidence_threshold: number;
    new_relayers: Relayer[];
  };
} | {
  cosmos_to_xrpl_transfer: {
    amount: Uint128;
    currency: string;
    issuer: string;
    max_amount?: Uint128 | null;
    recipient: string;
    sender: Addr;
  };
};
export interface PendingOperationsResponse {
  last_key?: number | null;
  operations: Operation[];
}
export interface Operation {
  account_sequence?: number | null;
  id: string;
  operation_type: OperationType;
  signatures: Signature[];
  ticket_sequence?: number | null;
  version: number;
  xrpl_base_fee: number;
}
export interface Signature {
  relayer_cosmos_address: Addr;
  signature: string;
}
export interface PendingRefundsResponse {
  last_key?: [Addr, string] | null;
  pending_refunds: PendingRefund[];
}
export interface PendingRefund {
  coin: Coin;
  id: string;
  xrpl_tx_hash?: string | null;
}
export type Boolean = boolean;
export interface ProcessedTxsResponse {
  last_key?: string | null;
  processed_txs: string[];
}
export interface ProhibitedXrplAddressesResponse {
  prohibited_xrpl_addresses: string[];
}
export interface TransactionEvidence {
  hash: string;
  relayer_addresses: Addr[];
}
export interface TransactionEvidencesResponse {
  last_key?: string | null;
  transaction_evidences: TransactionEvidence[];
}
export interface XrplTokensResponse {
  last_key?: string | null;
  tokens: XRPLToken[];
}
export interface XRPLToken {
  bridging_fee: Uint128;
  cosmos_denom: string;
  currency: string;
  issuer: string;
  max_holding_amount: Uint128;
  sending_precision: number;
  state: TokenState;
}