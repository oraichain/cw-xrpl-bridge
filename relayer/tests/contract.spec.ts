import { SimulateCosmWasmClient, HandleCustomMsgFunction, QueryCustomMsgFunction } from '@oraichain/cw-simulate';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { CwXrplClient, CwXrplTypes } from '../lib';
import { handleTokenFactory, queryTokenFactory } from '../src/cw-simulate/tokenfactory';
import { generateXrplAddress, generateXrplPubkey } from '../src/utils';

const handleCustomMsg: HandleCustomMsgFunction = async (sender, msg) => {
  let response = handleTokenFactory(client, sender, msg);
  if (response) return response;
};

const queryCustomMsg: QueryCustomMsgFunction = (request) => {
  let response = queryTokenFactory([], request);
  if (response) return response;
};

const client = new SimulateCosmWasmClient({ chainId: 'Oraichain', bech32Prefix: 'orai', handleCustomMsg, queryCustomMsg });
const receiverAddress = 'orai1e9rxz3ssv5sqf4n23nfnlh4atv3uf3fs5wgm66';
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

    await cwXrpl.createCosmosToken({
      subdenom: 'UTEST',
      decimals: 6,
      initialBalances: [
        {
          address: receiverAddress,
          amount: '100000000'
        }
      ],
      symbol: 'TEST',
      description: 'description'
    });

    const denom = `factory/${tokenFactoryAddr}/UTEST`;
    const balance = await client.getBalance(receiverAddress, denom);
    console.log(balance);
  });
});
