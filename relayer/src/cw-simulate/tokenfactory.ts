import { Coin, coins } from '@cosmjs/amino';
import { toBinary } from '@cosmjs/cosmwasm-stargate';
import { Ok, Err, TokenFactoryMsg, Metadata, SimulateCosmWasmClient, CosmosMsg, Result, AppResponse, QueryMessage, TokenFactoryQuery } from '@oraichain/cw-simulate';

const ADMIN: { [key: string]: string } = {};
const DENOMS_BY_CREATOR: { [key: string]: string[] } = {};
const METADATA: { [key: string]: Metadata } = {};

export const handleTokenFactory = async (client: SimulateCosmWasmClient, sender: string, msg: CosmosMsg): Promise<Result<AppResponse, string>> => {
  if ('custom' in msg && 'token' in msg.custom) {
    const tokenfactoryMsgOptions = (msg.custom as TokenFactoryMsg).token;

    if ('create_denom' in tokenfactoryMsgOptions) {
      const { subdenom, metadata } = tokenfactoryMsgOptions.create_denom;
      const newTokenDenom = `factory/${sender}/${subdenom}`;
      if (ADMIN[newTokenDenom]) {
        return Err('token exists');
      }

      ADMIN[newTokenDenom] = sender;

      const denoms = DENOMS_BY_CREATOR[sender] ?? [];
      denoms.push(newTokenDenom);
      DENOMS_BY_CREATOR[sender] = denoms;

      if (metadata) {
        METADATA[newTokenDenom] = metadata;
      }

      let data = toBinary({ new_token_denom: newTokenDenom });
      return Ok({
        data,
        events: []
      });
    }

    if ('mint_tokens' in tokenfactoryMsgOptions) {
      const { denom, amount, mint_to_address } = tokenfactoryMsgOptions.mint_tokens;

      // ensure we are admin of this denom (and it exists)
      const admin = ADMIN[denom];
      if (!admin) {
        return Err('token does not exist');
      }

      if (admin != sender) {
        return Err('sender is not token admin');
      }

      client.app.bank.mint(mint_to_address, coins(amount, denom));
      return Ok({ events: [], data: null });
    }

    if ('burn_tokens' in tokenfactoryMsgOptions) {
      const { denom, amount, burn_from_address } = tokenfactoryMsgOptions.burn_tokens;

      return client.app.bank.handleMsg(burn_from_address, { burn: { amount: coins(amount, denom) } });
    }

    if ('force_transfer' in tokenfactoryMsgOptions) {
      const { denom, amount, from_address, to_address } = tokenfactoryMsgOptions.force_transfer;
      return client.app.bank.handleMsg(from_address, { send: { to_address, amount: coins(amount, denom) } });
    }
  }
};

export const queryTokenFactory = (fee: Coin[], request: QueryMessage) => {
  if ('custom' in request && 'token' in request.custom) {
    const tokenfactoryQueryEnum = (request.custom as TokenFactoryQuery).token;

    if ('full_denom' in tokenfactoryQueryEnum) {
      const { creator_addr, subdenom } = tokenfactoryQueryEnum.full_denom;
      const denom = `factory/${creator_addr}/${subdenom}`;
      return { denom };
    }

    if ('metadata' in tokenfactoryQueryEnum) {
      const { denom } = tokenfactoryQueryEnum.metadata;
      const metadata = METADATA[denom];
      return { metadata };
    }

    if ('admin' in tokenfactoryQueryEnum) {
      const { denom } = tokenfactoryQueryEnum.admin;
      const admin = ADMIN[denom];
      return { admin };
    }

    if ('denoms_by_creator' in tokenfactoryQueryEnum) {
      const { creator } = tokenfactoryQueryEnum.denoms_by_creator;
      const denoms = DENOMS_BY_CREATOR[creator] ?? [];
      return { denoms };
    }

    if ('params' in tokenfactoryQueryEnum) {
      const params = {
        denom_creation_fee: fee
      };
      return { params };
    }
  }
};
