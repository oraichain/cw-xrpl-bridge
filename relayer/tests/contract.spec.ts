import { SimulateCosmWasmClient } from '@oraichain/cw-simulate';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { deriveAddress } from 'xrpl';
import { CwXrplClient, CwXrplTypes } from '../lib';
import { generateXrplAddress, generateXrplPubkey } from '../src/utils';

const client = new SimulateCosmWasmClient({ chainId: 'Oraichain', bech32Prefix: 'orai' });

const senderAddress = 'orai19xtunzaq20unp8squpmfrw8duclac22hd7ves2';

const deployTokenFactory = async () => {
  const tokenFactoryCode = readFileSync(resolve(__dirname, 'testdata', 'tokenfactory.wasm'));
  const { codeId } = await client.upload(senderAddress, tokenFactoryCode, 'auto');
  const { contractAddress } = await client.instantiate(senderAddress, codeId, {}, 'tokenfactory');
  return contractAddress;
};

describe('Test contract', () => {
  it('init contract', async () => {
    const wasmCode = readFileSync(resolve(__dirname, 'testdata', 'cw-xrpl.wasm'));
    const { codeId } = await client.upload(senderAddress, wasmCode, 'auto');
    const tokenFactoryAddr = await deployTokenFactory();
    const initMsg = {
      owner: senderAddress,
      relayers: [
        {
          cosmos_address: senderAddress,
          xrpl_address: generateXrplAddress(),
          xrpl_pub_key: generateXrplPubkey()
        }
      ],
      evidence_threshold: 1,
      used_ticket_sequence_threshold: 50,
      trust_set_limit_amount: '1000000000000000000',
      bridge_xrpl_address: generateXrplAddress(),
      xrpl_base_fee: 10,
      token_factory_addr: tokenFactoryAddr,
      issue_token: true
    } as CwXrplTypes.InstantiateMsg;

    const { contractAddress } = await client.instantiate(senderAddress, codeId, initMsg, 'cw-xrpl');

    const cwXrpl = new CwXrplClient(client, senderAddress, contractAddress);

    console.log(await cwXrpl.config());
  });
});
