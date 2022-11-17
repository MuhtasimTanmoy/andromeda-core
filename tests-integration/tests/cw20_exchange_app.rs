use std::str::FromStr;

use andromeda_app::app::AppComponent;
use andromeda_app_contract::mock::{
    mock_andromeda_app, mock_app_instantiate_msg, mock_claim_ownership_msg, mock_get_address_msg,
    mock_get_components_msg,
};
use andromeda_cw20::mock::{
    mock_andromeda_cw20, mock_cw20_instantiate_msg, mock_cw20_send, mock_get_cw20_balance,
    mock_minter,
};
use andromeda_cw20_exchange::mock::{
    mock_andromeda_cw20_exchange, mock_cw20_exchange_hook_purchase_msg,
    mock_cw20_exchange_instantiate_msg, mock_cw20_exchange_purchase_msg,
    mock_cw20_exchange_start_sale_msg,
};
use andromeda_fungible_tokens::cw20_exchange;
use andromeda_testing::mock::MockAndromeda;
use common::app::AndrAddress;
use cosmwasm_std::{
    coin, coins, to_binary, Addr, BalanceResponse as NativeBalanceResponse, BlockInfo, Coin,
    Decimal, Uint128,
};
use cw20::{BalanceResponse, Cw20Coin};
use cw721::{Expiration, OwnerOfResponse};
use cw_asset::AssetInfo;
use cw_multi_test::{App, Executor};

fn mock_app() -> App {
    App::new(|router, _api, storage| {
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("owner"),
                [coin(999999, "uandr")].to_vec(),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("buyer_one"),
                [coin(100, "uandr")].to_vec(),
            )
            .unwrap();
        router
            .bank
            .init_balance(
                storage,
                &Addr::unchecked("buyer_two"),
                [coin(100, "uandr")].to_vec(),
            )
            .unwrap();
    })
}

fn mock_andromeda(app: &mut App, admin_address: Addr) -> MockAndromeda {
    MockAndromeda::new(app, &admin_address)
}

#[test]
fn test_cw20_exchange_app() {
    let owner = Addr::unchecked("owner");
    let buyer_one = Addr::unchecked("buyer_one");
    let buyer_two = Addr::unchecked("buyer_two");

    let mut router = mock_app();
    let andr = mock_andromeda(&mut router, owner.clone());

    // Store contract codes
    let app_code_id = router.store_code(mock_andromeda_app());
    let cw20_code_id = router.store_code(mock_andromeda_cw20());
    let cw20_exchange_code_id = router.store_code(mock_andromeda_cw20_exchange());
    andr.store_code_id(&mut router, "app", app_code_id);
    andr.store_code_id(&mut router, "cw20", cw20_code_id);
    andr.store_code_id(&mut router, "cw20-exchange", cw20_exchange_code_id);

    // Generate App Components
    let initial_balances: Vec<Cw20Coin> = vec![Cw20Coin {
        address: owner.to_string(),
        amount: Uint128::from(1000000u128),
    }];
    let cw20_inst_msg = mock_cw20_instantiate_msg(
        "Test CW20",
        "TCW",
        6,
        initial_balances,
        Some(mock_minter(owner.to_string(), None)),
        None,
    );
    let cw20_app_component = AppComponent::new("1", "cw20", to_binary(&cw20_inst_msg).unwrap());

    let cw20_exchange_inst_msg =
        mock_cw20_exchange_instantiate_msg(cw20_app_component.clone().name);
    let cw20_exchange_app_component = AppComponent::new(
        "2",
        "cw20-exchange",
        to_binary(&cw20_exchange_inst_msg).unwrap(),
    );

    let app_components = vec![
        cw20_app_component.clone(),
        cw20_exchange_app_component.clone(),
    ];
    let app_inst_msg = mock_app_instantiate_msg(
        "CW20 Exchange App",
        app_components.clone(),
        andr.registry_address,
    );

    let app_addr = router
        .instantiate_contract(
            app_code_id,
            owner.clone(),
            &app_inst_msg,
            &[],
            "CW20 Exchange App",
            Some(owner.to_string()),
        )
        .unwrap();

    let components: Vec<AppComponent> = router
        .wrap()
        .query_wasm_smart(app_addr.clone(), &mock_get_components_msg())
        .unwrap();

    assert_eq!(components, app_components);

    router
        .execute_contract(
            owner.clone(),
            app_addr.clone(),
            &mock_claim_ownership_msg(None),
            &[],
        )
        .unwrap();

    let cw20_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(cw20_app_component.name),
        )
        .unwrap();
    let cw20_exchange_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(cw20_exchange_app_component.name),
        )
        .unwrap();

    // Create exchange asset
    let exchange_asset_initial_balances = vec![Cw20Coin {
        address: buyer_one.to_string(),
        amount: Uint128::from(100u128),
    }];
    let cw20_exchange_asset_inst_msg = mock_cw20_instantiate_msg(
        "Exchange Asset",
        "EACW",
        6,
        exchange_asset_initial_balances,
        None,
        None,
    );
    let cw20_exchange_asset_addr = router
        .instantiate_contract(
            cw20_code_id,
            buyer_one.clone(),
            &cw20_exchange_asset_inst_msg,
            &[],
            "Exchange Asset",
            None,
        )
        .unwrap();

    // Start Sale
    let sale_msg = mock_cw20_exchange_start_sale_msg(
        AssetInfo::Cw20(cw20_exchange_asset_addr.clone()),
        Uint128::from(10u128),
        None,
    );
    let send_msg = mock_cw20_send(
        cw20_exchange_addr.clone(),
        Uint128::from(20u128),
        to_binary(&sale_msg).unwrap(),
    );

    router
        .execute_contract(
            owner.clone(),
            Addr::unchecked(cw20_addr.clone()),
            &send_msg,
            &[],
        )
        .unwrap();

    // Purchase tokens with Exchange Asset
    let purchase_cw20_msg = mock_cw20_exchange_hook_purchase_msg(None);
    let purchase_cw20_send_msg = mock_cw20_send(
        cw20_exchange_addr.clone(),
        Uint128::from(100u128),
        to_binary(&purchase_cw20_msg).unwrap(),
    );

    router
        .execute_contract(
            buyer_one.clone(),
            cw20_exchange_asset_addr.clone(),
            &purchase_cw20_send_msg,
            &[],
        )
        .unwrap();

    // Validate balances post transaction
    let buyer_one_token_balance_query = mock_get_cw20_balance(buyer_one.clone());
    let buyer_one_token_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &buyer_one_token_balance_query)
        .unwrap();
    let buyer_one_exchange_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(
            cw20_exchange_asset_addr.clone(),
            &buyer_one_token_balance_query,
        )
        .unwrap();

    assert_eq!(buyer_one_token_balance.balance, Uint128::from(10u128));
    assert_eq!(buyer_one_exchange_balance.balance, Uint128::zero());

    let owner_token_balance_query = mock_get_cw20_balance(owner.clone());
    let owner_token_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &owner_token_balance_query)
        .unwrap();
    let owner_exchange_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_exchange_asset_addr.clone(), &owner_token_balance_query)
        .unwrap();

    assert_eq!(owner_token_balance.balance, Uint128::from(999980u128));
    assert_eq!(owner_exchange_balance.balance, Uint128::from(100u128));

    // Start sale with native tokens
    let sale_msg =
        mock_cw20_exchange_start_sale_msg(AssetInfo::native("uandr"), Uint128::from(10u128), None);
    let send_msg = mock_cw20_send(
        cw20_exchange_addr.clone(),
        Uint128::from(20u128),
        to_binary(&sale_msg).unwrap(),
    );

    router
        .execute_contract(
            owner.clone(),
            Addr::unchecked(cw20_addr.clone()),
            &send_msg,
            &[],
        )
        .unwrap();

    // Purchase wih native denomination
    let purchase_native_msg = mock_cw20_exchange_purchase_msg(None);

    let owner_native_balance_pre = router.wrap().query_balance(owner.clone(), "uandr").unwrap();

    router
        .execute_contract(
            buyer_two.clone(),
            Addr::unchecked(cw20_exchange_addr.clone()),
            &purchase_native_msg,
            &coins(100u128, "uandr"),
        )
        .unwrap();

    // Validate balances post transaction
    let buyer_two_token_balance_query = mock_get_cw20_balance(buyer_two.clone());
    let buyer_two_token_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &buyer_two_token_balance_query)
        .unwrap();
    let buyer_two_native_balance = router
        .wrap()
        .query_balance(buyer_two.clone(), "uandr")
        .unwrap();

    assert_eq!(buyer_two_token_balance.balance, Uint128::from(10u128));
    assert_eq!(buyer_two_native_balance.amount, Uint128::zero());

    let owner_token_balance_query = mock_get_cw20_balance(owner.clone());
    let owner_token_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &owner_token_balance_query)
        .unwrap();
    let owner_native_balance_post = router.wrap().query_balance(owner.clone(), "uandr").unwrap();

    assert_eq!(owner_token_balance.balance, Uint128::from(999960u128));
    assert_eq!(
        owner_native_balance_post.amount,
        owner_native_balance_pre
            .amount
            .checked_add(Uint128::from(100u128))
            .unwrap()
    );
}
