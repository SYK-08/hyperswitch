pub mod address;
pub mod api_keys;
pub mod business_profile;
pub mod cache;
pub mod capture;
pub mod cards_info;
pub mod configs;
pub mod connector_response;
pub mod customers;
pub mod dispute;
pub mod ephemeral_key;
pub mod events;
pub mod file;
pub mod fraud_check;
pub mod locker_mock_up;
pub mod mandate;
pub mod merchant_account;
pub mod merchant_connector_account;
pub mod merchant_key_store;
pub mod payment_link;
pub mod payment_method;
pub mod payout_attempt;
pub mod payouts;
pub mod refund;
pub mod reverse_lookup;

use std::fmt::Debug;

use data_models::payments::{
    payment_attempt::PaymentAttemptInterface, payment_intent::PaymentIntentInterface,
};
use masking::PeekInterface;
use redis_interface::errors::RedisError;
use serde::de;
use storage_impl::{redis::kv_store::RedisConnInterface, MockDb};

use crate::{consts, errors::CustomResult, services::Store};

#[derive(PartialEq, Eq)]
pub enum StorageImpl {
    Postgresql,
    PostgresqlTest,
    Mock,
}

#[async_trait::async_trait]
pub trait StorageInterface:
    Send
    + Sync
    + dyn_clone::DynClone
    + address::AddressInterface
    + api_keys::ApiKeyInterface
    + configs::ConfigInterface
    + capture::CaptureInterface
    + connector_response::ConnectorResponseInterface
    + customers::CustomerInterface
    + dispute::DisputeInterface
    + ephemeral_key::EphemeralKeyInterface
    + events::EventInterface
    + file::FileMetadataInterface
    + fraud_check::FraudCheckInterface
    + locker_mock_up::LockerMockUpInterface
    + mandate::MandateInterface
    + merchant_account::MerchantAccountInterface
    + merchant_connector_account::ConnectorAccessToken
    + merchant_connector_account::MerchantConnectorAccountInterface
    + PaymentAttemptInterface
    + PaymentIntentInterface
    + payment_method::PaymentMethodInterface
    + scheduler::SchedulerInterface
    + payout_attempt::PayoutAttemptInterface
    + payouts::PayoutsInterface
    + refund::RefundInterface
    + reverse_lookup::ReverseLookupInterface
    + cards_info::CardsInfoInterface
    + merchant_key_store::MerchantKeyStoreInterface
    + MasterKeyInterface
    + payment_link::PaymentLinkInterface
    + RedisConnInterface
    + business_profile::BusinessProfileInterface
    + 'static
{
    fn get_scheduler_db(&self) -> Box<dyn scheduler::SchedulerInterface>;
}

pub trait MasterKeyInterface {
    fn get_master_key(&self) -> &[u8];
}

impl MasterKeyInterface for Store {
    fn get_master_key(&self) -> &[u8] {
        self.master_key().peek()
    }
}

/// Default dummy key for MockDb
impl MasterKeyInterface for MockDb {
    fn get_master_key(&self) -> &[u8] {
        &[
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ]
    }
}

#[async_trait::async_trait]
impl StorageInterface for Store {
    fn get_scheduler_db(&self) -> Box<dyn scheduler::SchedulerInterface> {
        Box::new(self.clone())
    }
}

#[async_trait::async_trait]
impl StorageInterface for MockDb {
    fn get_scheduler_db(&self) -> Box<dyn scheduler::SchedulerInterface> {
        Box::new(self.clone())
    }
}

pub async fn get_and_deserialize_key<T>(
    db: &dyn StorageInterface,
    key: &str,
    type_name: &'static str,
) -> CustomResult<T, RedisError>
where
    T: serde::de::DeserializeOwned,
{
    use common_utils::ext_traits::ByteSliceExt;
    use error_stack::ResultExt;

    let bytes = db.get_key(key).await?;
    bytes
        .parse_struct(type_name)
        .change_context(redis_interface::errors::RedisError::JsonDeserializationFailed)
}

pub enum KvOperation<'a, S: serde::Serialize + Debug> {
    Hset((&'a str, String)),
    SetNx(S),
    HSetNx(&'a str, S),
    Get(&'a str),
    Scan(&'a str),
}

#[derive(router_derive::TryGetEnumVariant)]
#[error(RedisError(UnknownResult))]
pub enum KvResult<T: de::DeserializeOwned> {
    Get(T),
    Hset(()),
    SetNx(redis_interface::SetnxReply),
    HSetNx(redis_interface::HsetnxReply),
    Scan(Vec<T>),
}

pub async fn kv_wrapper<'a, T, S>(
    store: &Store,
    op: KvOperation<'a, S>,
    key: impl AsRef<str>,
) -> CustomResult<KvResult<T>, RedisError>
where
    T: de::DeserializeOwned,
    S: serde::Serialize + Debug,
{
    let redis_conn = store.get_redis_conn()?;

    let key = key.as_ref();
    let type_name = std::any::type_name::<T>();

    match op {
        KvOperation::Hset(value) => {
            redis_conn
                .set_hash_fields(key, value, Some(consts::KV_TTL))
                .await?;
            Ok(KvResult::Hset(()))
        }
        KvOperation::Get(field) => {
            let result = redis_conn
                .get_hash_field_and_deserialize(key, field, type_name)
                .await?;
            Ok(KvResult::Get(result))
        }
        KvOperation::Scan(pattern) => {
            let result: Vec<T> = redis_conn.hscan_and_deserialize(key, pattern, None).await?;
            Ok(KvResult::Scan(result))
        }
        KvOperation::HSetNx(field, value) => {
            let result = redis_conn
                .serialize_and_set_hash_field_if_not_exist(key, field, value, Some(consts::KV_TTL))
                .await?;
            Ok(KvResult::HSetNx(result))
        }
        KvOperation::SetNx(value) => {
            let result = redis_conn
                .serialize_and_set_key_if_not_exist(key, value, Some(consts::KV_TTL.into()))
                .await?;
            Ok(KvResult::SetNx(result))
        }
    }
}

dyn_clone::clone_trait_object!(StorageInterface);
