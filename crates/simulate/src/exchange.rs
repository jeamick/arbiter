#![warn(missing_docs)]
//! This module contains the `Exchange` and `Cfmm` traits that are used to describe the functionality of a contract that can be used to swap tokens.

use ethers::types::Address;

/// A trait that describes the functionality of any exchange.
pub trait Exchange {
    /// Returns the price listed on the exchange for a pair.
    fn get_price(&self, token_x: Address, token_y: Address) -> f64;
    /// Swaps a token for another token using the exchange's logic.
    fn swap(&self, token_in: Address, amount: f64);
}

/// Trait that uses the `Exchange` trait to describe the more detailed functionality of a CFMM.
pub trait Cfmm: Exchange {
    /// Returns the list of pools that the CFMM supports.
    fn get_pools(&self) -> Vec<String>;
    /// Lets a user add liquidity to a pool.
    fn add_liquidity(&self, token: &str, amount: f64);
    /// Lets a user remove liquidity from a pool.
    fn remove_liquidity(&self, token: &str, amount: f64);
}

#[cfg(test)]
mod tests {
    use bindings;
    use ethers::{
        prelude::{BaseContract, H256, U256},
        types::Address,
    };
    use revm::primitives::{ruint::Uint, B160};

    use crate::{
        agent::Agent,
        environment::{recast_address, SimulationContract, SimulationManager},
    };
    #[test]
    fn test_swap_from_x_liquid_exchange() {
        // Set up the execution manager and a user address.
        let mut manager = SimulationManager::default();
        let user_address = B160::from_low_u64_be(1);
        manager.create_user(user_address);

        // First we create arbiter token x and arbiter token y, then deploy LiquidExchange.
        // Get the general arbiter_token bytecode
        let arbiter_token = SimulationContract::new(
            BaseContract::from(bindings::arbiter_token::ARBITERTOKEN_ABI.clone()),
            bindings::arbiter_token::ARBITERTOKEN_BYTECODE
                .clone()
                .into_iter()
                .collect(),
        );

        // Deploy token_x
        let name = "Token X";
        let symbol = "TKNX";
        let args = (name.to_string(), symbol.to_string());
        let token_x = manager.deploy(&arbiter_token, args);

        // Mint token_x to the user
        let mint_amount = 20; // 20 token_x in ether units (1e18)
        let args = (recast_address(user_address), U256::from(mint_amount));
        let call_data = token_x
            .base_contract
            .encode("mint", args)
            .unwrap()
            .into_iter()
            .collect();
        manager.call_contract(&token_x, call_data, Uint::from(0));

        // Check that the user has the right amount of token_x
        let call_data = token_x
            .base_contract
            .encode("balanceOf", recast_address(user_address))
            .unwrap()
            .into_iter()
            .collect();
        let execution_result = manager.call_contract(&token_x, call_data, Uint::from(0)); // Call the 'balanceOf' function.
        let value = manager.unpack_execution(execution_result);
        let response: U256 = token_x
            .base_contract
            .decode_output("balanceOf", value)
            .unwrap();
        println!("User has {} token_x", response);
        assert_eq!(response, U256::from(mint_amount));

        // Deploy token_y
        let name = "Token Y";
        let symbol = "TKNY";
        let args = (name.to_string(), symbol.to_string());
        let token_y = manager.deploy(&arbiter_token, args);

        // Increase the manager's allowance for token_y.
        let args = (recast_address(manager.address), U256::from(mint_amount));
        let call_data = token_y
            .base_contract
            .encode("increaseAllowance", args)
            .unwrap()
            .into_iter()
            .collect();
        let execution_result = manager.call_contract(&token_y, call_data, Uint::from(0));
        let value = manager.unpack_execution(execution_result);
        let value: bool = token_y
            .base_contract
            .decode_output("increaseAllowance", value)
            .unwrap();
        println!("output of increaseAllowance for manager: {:#?}", value);

                // Increase the user's allowance for token_x.
                let args = (recast_address(user_address), U256::from(mint_amount));
                let call_data = token_x
                    .base_contract
                    .encode("increaseAllowance", args)
                    .unwrap()
                    .into_iter()
                    .collect();
                let execution_result = manager.call_contract(&token_x, call_data, Uint::from(0));
                let value = manager.unpack_execution(execution_result);
                let value: bool = token_x
                    .base_contract
                    .decode_output("increaseAllowance", value)
                    .unwrap();
                println!("output of increaseAllowance for user: {:#?}", value);

        // Deploy LiquidExchange
        let initial_price = 1000;
        let liquid_exchange = SimulationContract::new(
            BaseContract::from(bindings::liquid_exchange::LIQUIDEXCHANGE_ABI.clone()),
            bindings::liquid_exchange::LIQUIDEXCHANGE_BYTECODE
                .clone()
                .into_iter()
                .collect(),
        );
        let args = (
            recast_address(token_x.address.unwrap()),
            recast_address(token_y.address.unwrap()),
            U256::from(initial_price),
        );
        let le_xy = manager.deploy(&liquid_exchange, args);

        // Let the user call the swap function where we trade in token x for token y
        let swap_amount = mint_amount/2; // Swap half of the amount we minted
        let call_data = le_xy
            .base_contract
            .encode(
                "swap",
                (
                    recast_address(token_x.address.unwrap()),
                    U256::from(swap_amount),
                ),
            )
            .unwrap()
            .into_iter()
            .collect();
        let execution_result = manager.call_contract(&le_xy, call_data, Uint::from(0));
        let value = manager.unpack_execution(execution_result);
        println!("value: {:#?}", value);

        // Check the event log for the amount_out
        let logs = manager.read_logs();
        println!("logs: {:#?}", logs);
        let log_topics: Vec<H256> = logs.clone()[0]
            .topics
            .clone()
            .into_iter()
            .map(|x| H256::from_slice(x.as_slice()))
            .collect();
        let log_data = logs[0].data.clone().into();
        let log_output = le_xy
            .base_contract
            .decode_event::<(Address, U256, Address, U256)>("SwapOccured", log_topics, log_data)
            .unwrap();
        println!("log output: {:#?}", log_output);
        println!("log entry for amount_out: {:#?}", log_output.3);
        assert_eq!(log_output.3, U256::from(initial_price * swap_amount));
        

        // Check that the user received funds in token_y
        let call_data = token_y
            .base_contract
            .encode("balanceOf", recast_address(user_address))
            .unwrap()
            .into_iter()
            .collect();
        let execution_result = manager.call_contract(&token_y, call_data, Uint::from(0)); // Call the 'balanceOf' function.
        let value = manager.unpack_execution(execution_result);
        let response: U256 = token_y
            .base_contract
            .decode_output("balanceOf", value)
            .unwrap();
        println!("User has {} token_y after swap", response);
    }


    // TODO: Test that only admin can access admin function (change mint function to only admin)
}