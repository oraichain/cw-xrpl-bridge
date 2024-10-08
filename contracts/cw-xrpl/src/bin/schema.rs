use cosmwasm_schema::write_api;
use cw_xrpl::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};

//run cargo schema to generate
fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
        migrate: MigrateMsg,
    }
}
