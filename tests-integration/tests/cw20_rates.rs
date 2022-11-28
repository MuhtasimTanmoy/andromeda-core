use andromeda_app::app::AppComponent;
use andromeda_app_contract::mock::{
    mock_andromeda_app, mock_app_instantiate_msg, mock_claim_ownership_msg, mock_get_address_msg,
    mock_get_components_msg,
};
use andromeda_cw20::mock::{
    mock_andromeda_cw20, mock_cw20_instantiate_msg, mock_cw20_send, mock_cw20_transfer,
    mock_get_cw20_balance,
};
use andromeda_cw20_exchange::mock::{
    mock_andromeda_cw20_exchange, mock_cw20_exchange_instantiate_msg,
    mock_cw20_exchange_start_sale_msg,
};
use andromeda_modules::rates::{PercentRate, Rate, RateInfo};
use andromeda_rates::mock::{
    mock_add_exemption_msg, mock_andromeda_rates, mock_rates_instantiate_msg,
};
use andromeda_testing::mock::MockAndromeda;
use common::{
    ado_base::{modules::Module, recipient::Recipient},
    app::AndrAddress,
};
use cosmwasm_std::{to_binary, Addr, Decimal, Uint128};
use cw20::{BalanceResponse, Cw20Coin};
use cw_asset::AssetInfo;
use cw_multi_test::{App, Executor};

fn mock_app() -> App {
    App::new(|_router, _api, _storage| {})
}

fn mock_andromeda(app: &mut App, admin_address: Addr) -> MockAndromeda {
    MockAndromeda::new(app, &admin_address)
}

#[test]
fn test_cw20_rates() {
    let owner = Addr::unchecked("owner");
    let exempt_buyer = Addr::unchecked("exempt_buyer");
    let buyer = Addr::unchecked("buyer");
    let buyer_two = Addr::unchecked("buyer_two");

    let mut router = mock_app();
    let andr = mock_andromeda(&mut router, owner.clone());

    // Store contract codes
    let cw20_code_id = router.store_code(mock_andromeda_cw20());
    let rates_code_id = router.store_code(mock_andromeda_rates());
    let exchange_code_id = router.store_code(mock_andromeda_cw20_exchange());
    let app_code_id = router.store_code(mock_andromeda_app());
    andr.store_code_id(&mut router, "cw20", cw20_code_id);
    andr.store_code_id(&mut router, "rates", rates_code_id);
    andr.store_code_id(&mut router, "cw20-exchange", exchange_code_id);
    andr.store_code_id(&mut router, "app", app_code_id);

    let rates = vec![RateInfo {
        rate: Rate::Percent(PercentRate {
            percent: Decimal::percent(1),
        }),
        is_additive: false,
        description: None,
        recipients: vec![Recipient::Addr(owner.to_string())],
    }];
    let rates_init_msg = mock_rates_instantiate_msg(rates);
    let rates_app_component = AppComponent::new("1", "rates", to_binary(&rates_init_msg).unwrap());

    let initial_balances = vec![
        Cw20Coin {
            amount: Uint128::from(10000u128),
            address: exempt_buyer.to_string(),
        },
        Cw20Coin {
            amount: Uint128::from(10000u128),
            address: buyer.to_string(),
        },
        Cw20Coin {
            amount: Uint128::from(10000u128),
            address: buyer_two.to_string(),
        },
        Cw20Coin {
            amount: Uint128::from(200u128),
            address: owner.to_string(),
        },
    ];
    let modules = vec![Module {
        address: AndrAddress {
            identifier: rates_app_component.name.clone(),
        },
        module_type: "rates".to_string(),
        is_mutable: false,
    }];
    let cw20_init_msg = mock_cw20_instantiate_msg(
        "CW20 Tokens",
        "CWT",
        0,
        initial_balances,
        None,
        Some(modules),
    );
    let cw20_app_component = AppComponent::new("2", "cw20", to_binary(&cw20_init_msg).unwrap());

    let exchange_init_msg = mock_cw20_exchange_instantiate_msg(cw20_app_component.name.clone());
    let exchange_app_component =
        AppComponent::new("3", "cw20-exchange", to_binary(&exchange_init_msg).unwrap());
    let unexempt_exchange_app_component =
        AppComponent::new("4", "cw20-exchange", to_binary(&exchange_init_msg).unwrap());

    let app_components = vec![
        cw20_app_component.clone(),
        rates_app_component.clone(),
        exchange_app_component.clone(),
        unexempt_exchange_app_component.clone(),
    ];
    let app_init_msg = mock_app_instantiate_msg(
        "CW20 Rates App".to_string(),
        app_components.clone(),
        andr.registry_address.to_string(),
    );

    let app_addr = router
        .instantiate_contract(
            app_code_id,
            owner.clone(),
            &app_init_msg,
            &[],
            "CW20 Rates App",
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

    let exchange_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(exchange_app_component.name),
        )
        .unwrap();
    let unexempt_exchange_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(unexempt_exchange_app_component.name),
        )
        .unwrap();
    let cw20_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(cw20_app_component.name),
        )
        .unwrap();
    let rates_addr: String = router
        .wrap()
        .query_wasm_smart(
            app_addr.clone(),
            &mock_get_address_msg(rates_app_component.name),
        )
        .unwrap();

    // Add Exemptions
    let exemptions = vec![exempt_buyer.clone(), Addr::unchecked(exchange_addr.clone())];
    for addr in exemptions {
        let add_exemption_msg = mock_add_exemption_msg(addr);
        router
            .execute_contract(
                owner.clone(),
                Addr::unchecked(rates_addr.clone()),
                &add_exemption_msg,
                &[],
            )
            .unwrap();
    }

    // Send exempt
    let start_sale = mock_cw20_exchange_start_sale_msg(
        AssetInfo::Native("uandr".to_string()),
        Uint128::one(),
        None,
    );
    let msg = mock_cw20_send(
        exchange_addr.to_string(),
        Uint128::from(100u128),
        to_binary(&start_sale).unwrap(),
    );
    router
        .execute_contract(owner.clone(), Addr::unchecked(cw20_addr.clone()), &msg, &[])
        .unwrap();

    let receiver_balance_query = mock_get_cw20_balance(exchange_addr.clone());
    let receiver_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &receiver_balance_query)
        .unwrap();
    assert_eq!(receiver_balance.balance, Uint128::from(100u128));

    let owner_balance_query = mock_get_cw20_balance(owner.clone());
    let owner_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &owner_balance_query)
        .unwrap();
    assert_eq!(owner_balance.balance, Uint128::from(100u128));

    // Send unexempt
    let start_sale = mock_cw20_exchange_start_sale_msg(
        AssetInfo::Native("uandr".to_string()),
        Uint128::one(),
        None,
    );
    let msg = mock_cw20_send(
        unexempt_exchange_addr.to_string(),
        Uint128::from(100u128),
        to_binary(&start_sale).unwrap(),
    );
    router
        .execute_contract(owner.clone(), Addr::unchecked(cw20_addr.clone()), &msg, &[])
        .unwrap();

    let receiver_balance_query = mock_get_cw20_balance(unexempt_exchange_addr.clone());
    let receiver_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &receiver_balance_query)
        .unwrap();
    assert_eq!(receiver_balance.balance, Uint128::from(99u128));

    let owner_balance_query = mock_get_cw20_balance(owner.clone());
    let owner_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &owner_balance_query)
        .unwrap();
    assert_eq!(owner_balance.balance, Uint128::from(1u128));

    // Transfer unexempt
    let msg = mock_cw20_transfer(buyer_two.to_string(), Uint128::from(100u128));
    router
        .execute_contract(buyer.clone(), Addr::unchecked(cw20_addr.clone()), &msg, &[])
        .unwrap();

    let receiver_balance_query = mock_get_cw20_balance(buyer_two.clone());
    let receiver_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &receiver_balance_query)
        .unwrap();
    assert_eq!(receiver_balance.balance, Uint128::from(10099u128));

    let owner_balance_query = mock_get_cw20_balance(owner.clone());
    let owner_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &owner_balance_query)
        .unwrap();
    assert_eq!(owner_balance.balance, Uint128::from(2u128));

    // Transfer exempt address
    let msg = mock_cw20_transfer(exempt_buyer.to_string(), Uint128::from(100u128));
    router
        .execute_contract(buyer.clone(), Addr::unchecked(cw20_addr.clone()), &msg, &[])
        .unwrap();

    let receiver_balance_query = mock_get_cw20_balance(exempt_buyer.clone());
    let receiver_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &receiver_balance_query)
        .unwrap();
    assert_eq!(receiver_balance.balance, Uint128::from(10100u128));

    let owner_balance_query = mock_get_cw20_balance(owner.clone());
    let owner_balance: BalanceResponse = router
        .wrap()
        .query_wasm_smart(cw20_addr.clone(), &owner_balance_query)
        .unwrap();
    assert_eq!(owner_balance.balance, Uint128::from(2u128));
}
