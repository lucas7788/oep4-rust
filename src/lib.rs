#![no_std]
#![feature(proc_macro_hygiene)]
extern crate ontio_std as ostd;
use ostd::abi::{EventBuilder, Sink, Source};
use ostd::macros::base58;
use ostd::prelude::*;
use ostd::types::u128_to_neo_bytes;
use ostd::{database, runtime};

const KEY_TOTAL_SUPPLY: &[u8] = b"total_supply";
const NAME: &str = "wasm_token";
const SYMBOL: &str = "WTK";
const TOTAL_SUPPLY: U128 = 100_000_000_000;
const DECIMAL_MULTIPLIER: U128 = 100_000_000;

const KEY_BALANCE: &[u8] = b"01";
const KEY_APPROVE: &[u8] = b"02";

const ADMIN: Address = base58!("AbtTQJYKfQxq4UdygDsbLVjE8uRrJ2H3tP");

/**
     Initializes the contract
*/
fn initialize() -> bool {
    assert_eq!(total_supply(), 0);
    assert!(runtime::check_witness(&ADMIN));
    let total = TOTAL_SUPPLY.checked_mul(DECIMAL_MULTIPLIER).unwrap();
    database::put(KEY_TOTAL_SUPPLY, total);
    database::put(utils::gen_balance_key(&ADMIN), total);
    true
}
/**
    Returns the balance for the given address
    :param address: The address to check
*/
fn balance_of(addr: &Address) -> U128 {
    database::get(utils::gen_balance_key(addr)).unwrap_or(0)
}

/**
    Transfers an amount of tokens from from_acct to to_acct
    :param from_address: The address sending the tokens
    :param to_address: The address receiving the tokens
    :param amount: The amount being transferred
    Returns True on success, otherwise raises an exception
*/
fn transfer(from: &Address, to: &Address, amount: U128) -> bool {
    assert!(runtime::check_witness(from));
    let frmbal = balance_of(from);
    let tobal = balance_of(to);
    if amount == 0 || frmbal < amount {
        return false;
    }
    if frmbal == amount {
        database::delete(utils::gen_balance_key(from))
    } else {
        database::put(utils::gen_balance_key(from), frmbal - amount);
    }
    database::put(utils::gen_balance_key(to), tobal + amount);
    EventBuilder::new()
        .bytearray("transfer".as_bytes())
        .bytearray(from.as_bytes())
        .bytearray(to.as_bytes())
        .bytearray(u128_to_neo_bytes(amount).as_slice())
        .notify();
    true
}

/**
    Allows the transferring of tokens from multiple addresses to multiple other addresses with multiple amounts of tokens
    :param args: An array of arrays in the format of  [[from, to, amount] ... [from_n, to_n, amount_n]]
    Returns True on success, otherwise raises an exception
*/
fn transfer_multi(states: &[(&Address, &Address, U128)]) -> bool {
    for &state in states.iter() {
        assert!(transfer(state.0, state.1, state.2));
    }
    true
}
/**
    Allows the spender to transfer tokens on behalf of the owner
    :param owner: The address granting permissions
    :param spender: The address that will be able to transfer the owner's tokens
    :param amount: The amount of tokens being enabled for transfer
    Returns True on success, otherwise raises an exception
*/
fn approve(owner: &Address, spender: &Address, amount: U128) -> bool {
    assert!(runtime::check_witness(owner));
    assert!(amount <= balance_of(owner));
    let allowance = allowance(owner, spender);
    let approve = amount + allowance;
    database::put(utils::gen_approve_key(owner, spender), approve);
    EventBuilder::new()
        .bytearray("approve".as_bytes())
        .bytearray(owner.as_bytes())
        .bytearray(spender.as_bytes())
        .bytearray(u128_to_neo_bytes(amount).as_slice())
        .notify();
    true
}

/**
    Gets the amount of tokens that the spender is allowed to spend on behalf of the owner
    :param owner: The owner address
    :param spender:  The spender address
*/
fn allowance(owner: &Address, spender: &Address) -> U128 {
    database::get(utils::gen_approve_key(owner, spender)).unwrap_or(0)
}

/**
    The spender address sends amount of tokens from the from_address to the to_address
    :param spender: The address sending the funds
    :param from_address: The address whose funds are being sent
    :param to_address: The receiving address
    :param amount: The amounts of tokens being transferred
    Returns True on success, otherwise raises an exception
*/
fn transfer_from(spender: &Address, from: &Address, amount: U128) -> bool {
    assert!(runtime::check_witness(spender));
    let allowance = allowance(from, spender);
    assert!(amount <= allowance);
    let from_balance = balance_of(from);
    assert!(from_balance >= amount);
    if amount == allowance {
        database::delete(utils::gen_approve_key(from, spender));
    } else {
        database::put(utils::gen_approve_key(from, spender), allowance - amount);
    }

    let spender_balance = balance_of(spender);
    database::put(utils::gen_balance_key(spender), spender_balance + amount);
    if from_balance == amount {
        database::delete(utils::gen_balance_key(from));
    } else {
        database::put(utils::gen_balance_key(from), from_balance - amount);
    }
    true
}
/**
    Returns the total supply of the token
*/
fn total_supply() -> U128 {
    database::get(KEY_TOTAL_SUPPLY).unwrap_or(0)
}

#[no_mangle]
pub fn invoke() {
    let input = runtime::input();
    let mut source = Source::new(&input);
    let action: &[u8] = source.read().unwrap();
    let mut sink = Sink::new(12);
    match action {
        b"init" => sink.write(initialize()),
        b"name" => sink.write(NAME),
        b"symbol" => sink.write(SYMBOL),
        b"decimal" => sink.write(DECIMAL_MULTIPLIER),
        b"totalSupply" => sink.write(total_supply()),
        b"balanceOf" => {
            let addr = source.read().unwrap();
            sink.write(balance_of(addr));
        }
        b"transfer" => {
            let (from, to, amount) = source.read().unwrap();
            sink.write(transfer(from, to, amount));
        }
        b"transferMulti" => {
            let states: Vec<(&Address, &Address, U128)> = source.read().unwrap();
            sink.write(transfer_multi(states.as_slice()));
        }
        b"approve" => {
            let (owner, spender, amount) = source.read().unwrap();
            sink.write(approve(owner, spender, amount));
        }
        b"allowance" => {
            let (owner, spender) = source.read().unwrap();
            sink.write(allowance(owner, spender));
        }
        b"transferFrom" => {
            let (spender, from, amount) = source.read().unwrap();
            sink.write(transfer_from(spender, from, amount));
        }
        _ => panic!("unsupported action!"),
    }

    runtime::ret(sink.bytes())
}

mod utils {
    use super::*;
    pub fn gen_balance_key(addr: &Address) -> Vec<u8> {
        [KEY_BALANCE, addr.as_ref()].concat()
    }
    pub fn gen_approve_key(owner: &Address, spender: &Address) -> Vec<u8> {
        [KEY_APPROVE, owner.as_ref(), spender.as_ref()].concat()
    }
}

#[cfg(test)]
mod tests {
    extern crate ontio_std as ostd;
    use crate::ostd::abi::Decoder;
    use ostd::mock::build_runtime;
    use ostd::prelude::*;
    use ostd::types::Address;
    #[test]
    fn test_init() {
        let handle = build_runtime();
        handle.witness(&[crate::ADMIN]);
        assert!(crate::initialize());
        let total = crate::DECIMAL_MULTIPLIER * crate::TOTAL_SUPPLY;
        assert_eq!(crate::total_supply(), total);
        assert_eq!(crate::balance_of(&crate::ADMIN), total);

        let owner = Address::repeat_byte(1);
        let spender = Address::repeat_byte(2);
        let amount = 100 as U128;
        assert!(crate::transfer(&crate::ADMIN, &owner, amount));
        assert_eq!(crate::balance_of(&crate::ADMIN), total - amount);
        assert_eq!(crate::balance_of(&owner), amount);

        handle.witness(&[owner.clone()]);
        assert!(crate::approve(&owner, &spender, amount));
        assert_eq!(crate::allowance(&owner, &spender), amount);

        let amount2: U128 = 50;
        handle.witness(&[spender.clone()]);
        assert!(crate::transfer_from(&spender, &owner, amount2));
        assert_eq!(crate::allowance(&owner, &spender), amount - amount2);
        assert_eq!(crate::balance_of(&owner), amount - amount2);
        assert_eq!(crate::balance_of(&spender), amount2);

        let to1 = Address::repeat_byte(3);
        let to2 = Address::repeat_byte(4);
        let states = vec![(&crate::ADMIN, &to1, amount), (&crate::ADMIN, &to2, amount)];
        handle.witness(&[&crate::ADMIN]);
        assert!(crate::transfer_multi(states.as_slice()));

        assert_eq!(crate::balance_of(&crate::ADMIN), total - 3 * amount);
        assert_eq!(crate::balance_of(&to1), amount);
        assert_eq!(crate::balance_of(&to2), amount);
    }
}
