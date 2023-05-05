use crate::amp::{ADO_DB_KEY, VFS_KEY};
use crate::error::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{from_slice, Addr, QuerierWrapper};
use cw_storage_plus::Path;
use lazy_static::__Deref;
use serde::de::DeserializeOwned;
use std::str::from_utf8;

#[cw_serde]
pub struct AOSQuerier();

impl AOSQuerier {
    // namespace -> storage key
    // key_name -> item key
    // Taken from: https://github.com/KompleTeam/komple-framework/blob/387d333af03e794927b8ef8ac536d2a42ae7a1ff/packages/utils/src/storage.rs#L25
    pub fn get_map_storage_key(
        namespace: &str,
        key_bytes: &[&[u8]],
    ) -> Result<String, ContractError> {
        let namespace_bytes = namespace.as_bytes();
        let path: Path<Vec<u32>> = Path::new(namespace_bytes, key_bytes);
        let path_str = from_utf8(path.deref())?;
        Ok(path_str.to_string())
    }

    // To find the key value in storage, we need to construct a path to the key
    // For Map storage this key is generated with get_map_storage_key
    // For Item storage this key is the namespace value
    pub fn query_storage<T>(
        querier: &QuerierWrapper,
        addr: &Addr,
        key: &str,
    ) -> Result<Option<T>, ContractError>
    where
        T: DeserializeOwned,
    {
        let data = querier.query_wasm_raw(addr, key.as_bytes())?;
        match data {
            Some(data) => {
                let res = from_utf8(&data)?;
                let res = from_slice(res.as_bytes())?;
                Ok(Some(res))
            }
            None => Ok(None),
        }
    }

    pub fn ado_type_getter(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        code_id: u64,
    ) -> Result<String, ContractError> {
        let key = AOSQuerier::get_map_storage_key("ado_type", &[code_id.to_string().as_bytes()])?;
        let verify: Option<String> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;

        match verify {
            Some(ado_type) => Ok(ado_type),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    /// Checks if the code id exists in the ADODB by querying its raw storage for the code id's ado type
    pub fn verify_code_id(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        code_id: u64,
    ) -> Result<(), ContractError> {
        let key = AOSQuerier::get_map_storage_key("ado_type", &[code_id.to_string().as_bytes()])?;
        let verify: Option<String> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;

        if verify.is_some() {
            Ok(())
        } else {
            Err(ContractError::Unauthorized {})
        }
    }

    pub fn code_id_getter(
        querier: &QuerierWrapper,
        adodb_addr: &Addr,
        ado_type: &str,
    ) -> Result<u64, ContractError> {
        let key = AOSQuerier::get_map_storage_key("code_id", &[ado_type.as_bytes()])?;
        let verify: Option<u64> = AOSQuerier::query_storage(querier, adodb_addr, &key)?;

        match verify {
            Some(code_id) => Ok(code_id),
            None => Err(ContractError::InvalidAddress {}),
        }
    }

    /// Queries the kernel's raw storage for the VFS's address
    pub fn vfs_address_getter(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
    ) -> Result<Addr, ContractError> {
        AOSQuerier::kernel_address_getter(querier, kernel_addr, VFS_KEY)
    }

    /// Queries the kernel's raw storage for the ADODB's address
    pub fn adodb_address_getter(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
    ) -> Result<Addr, ContractError> {
        AOSQuerier::kernel_address_getter(querier, kernel_addr, ADO_DB_KEY)
    }

    /// Queries the kernel's raw storage for the VFS's address
    pub fn kernel_address_getter(
        querier: &QuerierWrapper,
        kernel_addr: &Addr,
        key: &str,
    ) -> Result<Addr, ContractError> {
        let key = AOSQuerier::get_map_storage_key("kernel_addresses", &[key.as_bytes()])?;
        let verify: Option<Addr> = AOSQuerier::query_storage(querier, kernel_addr, &key)?;
        match verify {
            Some(address) => Ok(address),
            None => Err(ContractError::InvalidAddress {}),
        }
    }
}
