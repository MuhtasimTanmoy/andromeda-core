use cosmwasm_std::{DepsMut, Env, Event, MessageInfo, QuerierWrapper};

use crate::{
    error::ContractError,
    modules::common::{add_payment, calculate_fee},
    modules::hooks::{MessageHooks, PaymentAttribute},
    modules::Rate,
    modules::{Module, ModuleDefinition},
    require,
    token::TransferAgreement,
};

use super::hooks::{HookResponse, ATTR_DESC, ATTR_PAYMENT};

pub const TAX_EVENT_ID: &str = "tax";

/// Struct used to define a Tax module.
/// This module will generate required payments upon an agreed transfer of any token.
pub struct Taxable {
    /// The rate of the tax
    pub rate: Rate,
    /// The receiving addresses of the tax fee
    pub receivers: Vec<String>,
    /// An optional description of the tax fee
    pub description: Option<String>,
}

impl MessageHooks for Taxable {
    /// Calculates the required tax fee for an agreed transfer and returns the required `BankMsg` to send the fee payments.
    /// Generates a tax payment event.
    /// **Any fees generated by this hook are paid for by the purchaser and are required to be sent by the purchaser upon sending a transfer message.**
    fn on_agreed_transfer(
        &self,
        deps: &DepsMut,
        _info: MessageInfo,
        env: Env,
        payments: &mut Vec<cosmwasm_std::BankMsg>,
        _owner: String,
        agreement: TransferAgreement,
    ) -> Result<HookResponse, ContractError> {
        let _contract_addr = env.contract.address;
        let rate = self.rate.validate(&deps.querier)?;
        let tax_amount = calculate_fee(rate, &agreement.amount)?;

        let mut resp = HookResponse::default();
        let mut event = Event::new(TAX_EVENT_ID);

        if let Some(desc) = self.description.clone() {
            event = event.add_attribute(ATTR_DESC, desc);
        }
        // No deduction of payment because the buyer pays the tax while royalties are paid by seller [ROY-01]/[TAX-02]
        for receiver in self.receivers.to_vec() {
            add_payment(payments, receiver.clone(), tax_amount.clone());
            event = event.add_attribute(
                ATTR_PAYMENT,
                PaymentAttribute {
                    receiver,
                    amount: tax_amount.clone(),
                }
                .to_string(),
            );
        }
        resp = resp.add_event(event);

        Ok(resp)
    }
}

impl Module for Taxable {
    /// Validates the tax module:
    /// * Tax must have at least one receiver
    /// * Tax rate must be non-zero
    /// * Any optional description provided cannot exceed 200 characters in length
    fn validate(
        &self,
        _modules: Vec<ModuleDefinition>,
        querier: &QuerierWrapper,
    ) -> Result<bool, ContractError> {
        require(
            !self.receivers.is_empty(),
            ContractError::NoReceivingAddress {},
        )?;
        self.rate.validate(querier)?;
        if self.description.clone().is_some() {
            require(
                self.description.clone().unwrap().len() <= 200,
                ContractError::ModuleDiscriptionTooLong {
                    msg: "Module description can be at most 200 characters long".to_string(),
                },
            )?;
        }

        Ok(true)
    }
    fn as_definition(&self) -> ModuleDefinition {
        ModuleDefinition::Taxable {
            rate: self.rate.clone(),
            receivers: self.receivers.clone(),
            description: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        coin, coins,
        testing::{mock_dependencies, mock_env, mock_info},
        BankMsg, Uint128,
    };

    use super::*;

    #[test]
    fn test_taxable_validate() {
        let mut deps = mock_dependencies(&[]);
        let t = Taxable {
            rate: Rate::Percent(2u128.into()),
            receivers: vec![String::default()],
            description: None,
        };

        assert!(t.validate(vec![], &deps.as_mut().querier).unwrap());

        let t_invalidtax = Taxable {
            rate: Rate::Percent(Uint128::zero()),
            receivers: vec![String::default()],
            description: None,
        };

        assert_eq!(
            t_invalidtax
                .validate(vec![], &deps.as_mut().querier)
                .unwrap_err(),
            ContractError::InvalidRate {}
        );

        let t_invalidrecv = Taxable {
            rate: Rate::Percent(2u128.into()),
            receivers: vec![],
            description: None,
        };

        assert_eq!(
            t_invalidrecv
                .validate(vec![], &deps.as_mut().querier)
                .unwrap_err(),
            ContractError::NoReceivingAddress {}
        );
    }

    #[test]

    fn test_taxable_on_agreed_transfer() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("sender", &[]);
        let env = mock_env();
        let receivers = vec![String::from("recv1"), String::from("recv2")];
        let t = Taxable {
            rate: Rate::Percent(3u128.into()),
            receivers,
            description: None,
        };

        let agreed_transfer_amount = coin(117, "uluna");
        let tax_amount = 4;
        let owner = String::from("owner");
        let purchaser = String::from("purchaser");
        let mut payments = vec![];

        t.on_agreed_transfer(
            &deps.as_mut(),
            info,
            env,
            &mut payments,
            owner,
            TransferAgreement {
                purchaser,
                amount: agreed_transfer_amount.clone(),
            },
        )
        .unwrap();

        assert_eq!(payments.len(), 2);

        let first_payment = BankMsg::Send {
            to_address: String::from("recv1"),
            amount: coins(tax_amount, &agreed_transfer_amount.denom),
        };
        let second_payment = BankMsg::Send {
            to_address: String::from("recv2"),
            amount: coins(tax_amount, &agreed_transfer_amount.denom),
        };

        assert_eq!(payments[0], first_payment);
        assert_eq!(payments[1], second_payment);
    }

    #[test]

    fn test_taxable_on_agreed_transfer_resp() {
        let mut deps = mock_dependencies(&[]);
        let info = mock_info("sender", &[]);
        let env = mock_env();
        let desc = "Some tax module";
        let receivers = vec![String::from("recv1"), String::from("recv2")];
        let t = Taxable {
            rate: Rate::Percent(1u128.into()),
            receivers,
            description: Some(desc.to_string()),
        };

        let agreed_transfer_amount = coin(100, "uluna");
        let owner = String::from("owner");
        let purchaser = String::from("purchaser");
        let mut payments = vec![];

        let resp = t
            .on_agreed_transfer(
                &deps.as_mut(),
                info,
                env,
                &mut payments,
                owner,
                TransferAgreement {
                    purchaser,
                    amount: agreed_transfer_amount.clone(),
                },
            )
            .unwrap();

        assert_eq!(resp.events.len(), 1);
        assert_eq!(resp.events[0].ty, "tax");
        assert_eq!(resp.events[0].attributes.len(), 3);
        assert_eq!(resp.events[0].attributes[0].key, ATTR_DESC);
        assert_eq!(resp.events[0].attributes[0].value, desc.to_string());
        assert_eq!(resp.events[0].attributes[1].key, ATTR_PAYMENT);
        assert_eq!(
            resp.events[0].attributes[1].value,
            PaymentAttribute {
                receiver: t.receivers[0].clone(),
                amount: calculate_fee(t.rate, &agreed_transfer_amount).unwrap()
            }
            .to_string()
        );
    }
}
