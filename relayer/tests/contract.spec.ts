import { SimulateCosmWasmClient } from '@oraichain/cw-simulate';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { CwXrplClient, CwXrplTypes } from '../lib';

const senderAddress = 'orai19xtunzaq20unp8squpmfrw8duclac22hd7ves2';

describe('Test contract', () => {
  const client = new SimulateCosmWasmClient({ chainId: 'Oraichain', bech32Prefix: 'orai' });
  const wasmCode = readFileSync(resolve(__dirname, 'testdata', 'cw-xrpl.wasm'));

  it('init contract', async () => {
    const { codeId } = await client.upload(senderAddress, wasmCode, 'auto');
    const { contractAddress } = await client.instantiate(
      senderAddress,
      codeId,
      {
        owner: senderAddress,
        relayers: [
          {
            cosmos_address: senderAddress,
            xrpl_address: 'xrpl_address',
            xrpl_pub_key: 'xrpl_pub_key'
          }
        ],
        evidence_threshold: 1,
        used_ticket_sequence_threshold: 50,
        trust_set_limit_amount: '1000000000000000000',
        bridge_xrpl_address: 'generate_xrpl_address',
        xrpl_base_fee: 10,
        token_factory_addr: 'token_factory_addr',
        issue_token: true
      } as CwXrplTypes.InstantiateMsg,
      'cw-xrpl'
    );

    const cwXrpl = new CwXrplClient(client, senderAddress, contractAddress);

    console.log(await cwXrpl.config());
  });
});
