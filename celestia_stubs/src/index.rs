extern crate derive_builder;
extern crate serde;
extern crate serde_json;
use core::future::Future;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use jsonrpsee::core::Error;
use core::pin::Pin;

pub type ArrayGnKkLVm4 = Vec<String>;
/// Integer7BU60JXWAsInteger
///
/// # Example
///
/// `42`
///
pub type Integer7BU60JXWAsInteger = i64;
/// StringBvkcVWUgAsString
///
/// # Example
///
/// `AAAAAAAAAAAAAAAAAAAAAAAAAAECAwQFBgcICRA=`
///
pub type StringBvkcVWUgAsString = String;
/// StringNayfGy6PAsString
///
/// # Example
///
/// `Bw==`
///
pub type StringNayfGy6PAsString = String;
pub type StringOfElfKS1AsString = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringOfElfKS1 {
    StringOfElfKS1AsString(StringOfElfKS1AsString),
}
/// ArrayP2Psh38GAsArray
///
/// # Example
///
/// `AAAAAAAAAAAAAAAAAAAAAAAAAAECAwQFBgcICRA=`
///
pub type ArrayP2Psh38GAsArray = (StringOfElfKS1);
pub type Object88OBAfX3AsObject = String;
pub type ArrayYTcMIOLC = serde_json::Value;
pub type ArrayWenhckXh = Vec<serde_json::Value>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Object88OBAfX3 {
    Object88OBAfX3AsObject(Object88OBAfX3AsObject),
}
/// ArrayXVPF8NrgAsArray
///
/// # Example
///
/// `[object Object]`
///
pub type ArrayXVPF8NrgAsArray = (Object88OBAfX3);
pub type AnyL9Fw4VUO = Blob;
pub type StringBA0Rq1IJ = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringBA0Rq1IJAnyL9Fw4VUO9A6PUlcT {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<Blob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commitment: Option<StringBA0Rq1IJ>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct Blob {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commitment: Option<String>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectXmpLeQAPAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<ObjectOfStringBA0Rq1IJAnyL9Fw4VUO9A6PUlcT>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commitment: Option<StringBA0Rq1IJ>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectXmpLeQAP {
    ObjectXmpLeQAPAsObject(ObjectXmpLeQAPAsObject),
}
/// ArrayI8QwXWBjAsArray
///
/// # Example
///
/// `[object Object]`
///
pub type ArrayI8QwXWBjAsArray = (ObjectXmpLeQAP);
pub type String2AHOqbcQ = String;
/// ObjectOq6Lbs0XAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOq6Lbs0XAsObject {
    #[serde(rename = "Fee", skip_serializing_if = "Option::is_none")]
    pub fee: Option<i64>,
    #[serde(rename = "GasLimit", skip_serializing_if = "Option::is_none")]
    pub gas_limit: Option<i64>,
}
/// StringRulx2YlQAsString
///
/// # Example
///
/// `badencodingv0.1`
///
pub type StringRulx2YlQAsString = String;
/// StringQrjJ8DPhAsString
///
/// # Example
///
/// `07`
///
pub type StringQrjJ8DPhAsString = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDW {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8J {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts: Option<ObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDW>,
}
pub type StringV0UwQeVu = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJString2AHOqbcQ81RUWTrX {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id_flag: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<StringV0UwQeVu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_address: Option<StringBA0Rq1IJ>,
}
pub type UnorderedSetOfObjectOfStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJString2AHOqbcQ81RUWTrXnxqhzBSe =
    Vec<ObjectOfStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJString2AHOqbcQ81RUWTrX>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfUnorderedSetOfObjectOfStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJString2AHOqbcQ81RUWTrXnxqhzBSeString2AHOqbcQString2AHOqbcQObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8JRu3DVAW1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<ObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8J>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<UnorderedSetOfObjectOfStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJString2AHOqbcQ81RUWTrXnxqhzBSe>,
}
pub type UnorderedSetOfStringBA0Rq1IJjh9Zs8EL = Vec<StringBA0Rq1IJ>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfUnorderedSetOfStringBA0Rq1IJjh9Zs8ELUnorderedSetOfStringBA0Rq1IJjh9Zs8ELBE2T5R7P
{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_roots: Option<UnorderedSetOfStringBA0Rq1IJjh9Zs8EL>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_roots: Option<UnorderedSetOfStringBA0Rq1IJjh9Zs8EL>,
}
pub type StringDoaGddGA = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfString2AHOqbcQString2AHOqbcQRRuuXHRJ {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<String2AHOqbcQ>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfObjectOfString2AHOqbcQString2AHOqbcQRRuuXHRJStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8JString2AHOqbcQStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJStringDoaGddGAStringBA0Rq1IJPuwa0B7M
{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_block_id:
        Option<ObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8J>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_results_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_validators_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposer_address: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<StringV0UwQeVu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validators_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<ObjectOfString2AHOqbcQString2AHOqbcQRRuuXHRJ>,
}
type AlwaysTrue = serde_json::Value;
pub type AnyZc4BnYi3 = serde_json::Value;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy23 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposer_priority: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pub_key: Option<AnyZc4BnYi3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voting_power: Option<String2AHOqbcQ>,
}
pub type UnorderedSetOfObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy232V8C0Y7P =
    Vec<ObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy23>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfUnorderedSetOfObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy232V8C0Y7PObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy23KDJSbC9F {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposer: Option<ObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy23>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validators: Option<UnorderedSetOfObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy232V8C0Y7P>,
}
/// ObjectIIoInpd7AsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectIIoInpd7AsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<ObjectOfUnorderedSetOfObjectOfStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJString2AHOqbcQ81RUWTrXnxqhzBSeString2AHOqbcQString2AHOqbcQObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8JRu3DVAW1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dah: Option<ObjectOfUnorderedSetOfStringBA0Rq1IJjh9Zs8ELUnorderedSetOfStringBA0Rq1IJjh9Zs8ELBE2T5R7P>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<ObjectOfObjectOfString2AHOqbcQString2AHOqbcQRRuuXHRJStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8JString2AHOqbcQStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJStringDoaGddGAStringBA0Rq1IJPuwa0B7M>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_set: Option<ObjectOfUnorderedSetOfObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy232V8C0Y7PObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy23KDJSbC9F>,
}
pub type ArrayHQjRkX0C = Vec<ObjectIIoInpd7AsObject>;

/// Array8BVxQ2XkAsArray
///
/// # Example
///
/// `admin`
///
pub type Array8BVxQ2XkAsArray = (StringOfElfKS1);
/// StringEprQ2TNCAsString
///
/// # Example
///
/// `string value`
///
pub type StringEprQ2TNCAsString = String;
/// String9UfShva3AsString
///
/// # Example
///
/// `CovLVG4fQcqUS6DmoMxAwVJGNW6PMzfwTG6BHW9NH9TLGHcbRfvPVc3JVhnufK3HTzStoTo`
///
pub type String9UfShva3AsString = String;
pub type ArrayRFsCAiJu = Vec<String9UfShva3AsString>;
/// StringLnSWZgcxAsString
///
/// # Example
///
/// `/celestia/mocha/ipfs/bitswap`
///
pub type StringLnSWZgcxAsString = String;
pub type UnorderedSetOfAnyZc4BnYi3IS0MYmHV = Vec<AnyZc4BnYi3>;
/// ObjectHWS0RFGbAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectHWS0RFGbAsObject {
    #[serde(rename = "Addrs", skip_serializing_if = "Option::is_none")]
    pub addrs: Option<UnorderedSetOfAnyZc4BnYi3IS0MYmHV>,
    #[serde(rename = "ID", skip_serializing_if = "Option::is_none")]
    pub id: Option<StringDoaGddGA>,
}
/// ObjectQ6MEBEIPAsObject
///
/// # Example
///
/// `celestia1377k5an3f94v6wyaceu0cf4nq6gk2jtpc46g7h`
///
pub type ObjectQ6MEBEIPAsObject = String;
/// StringI8Xo6AwhAsString
///
/// # Example
///
/// `celestiavaloper1q3v5cugc8cdpud87u4zwy0a74uxkk6u4q4gx4p`
///
pub type StringI8Xo6AwhAsString = String;
/// ObjectOPxuZea0AsObject
///
/// # Example
///
/// `42`
///
pub type ObjectOPxuZea0AsObject = String;
/// StringPZrCQoJgAsString
///
/// # Example
///
/// `celestia1377k5an3f94v6wyaceu0cf4nq6gk2jtpc46g7h`
///
pub type StringPZrCQoJgAsString = String;
/// ObjectFTcB5RWoAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectFTcB5RWoAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<AnyL9Fw4VUO>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<StringBA0Rq1IJ>,
}
/// BooleanIYY4Gv1XAsBoolean
///
/// # Example
///
/// `true`
///
pub type BooleanIYY4Gv1XAsBoolean = bool;
pub type BooleanVyG3AETh = bool;
pub type ObjectUDMjxfmN = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfString2AHOqbcQStringDoaGddGAString2AHOqbcQStringDoaGddGAString2AHOqbcQThvj5TPb
{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_type: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String2AHOqbcQ>,
}
pub type UnorderedSetOfObjectOfString2AHOqbcQStringDoaGddGAString2AHOqbcQStringDoaGddGAString2AHOqbcQThvj5TPbAdVUv4HX =
    Vec<ObjectOfString2AHOqbcQStringDoaGddGAString2AHOqbcQStringDoaGddGAString2AHOqbcQThvj5TPb>;
/// ObjectSVDfQMtaAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectSVDfQMtaAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catch_up_done: Option<BooleanVyG3AETh>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<ObjectUDMjxfmN>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_of_catchup: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_of_sampled_chain: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_running: Option<BooleanVyG3AETh>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_head_height: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<UnorderedSetOfObjectOfString2AHOqbcQStringDoaGddGAString2AHOqbcQStringDoaGddGAString2AHOqbcQThvj5TPbAdVUv4HX>,
}
pub type NullIUT54VvMAsNull = serde_json::Value;
/// ArrayNfMA4Xz1AsArray
///
/// # Example
///
/// `[object Object]`
///
pub type ArrayNfMA4Xz1AsArray = (Object88OBAfX3);
pub type TypeUnsupportedByJSONSchemaAsObject = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectGbKRBxC9AsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<ObjectOfUnorderedSetOfObjectOfStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJString2AHOqbcQ81RUWTrXnxqhzBSeString2AHOqbcQString2AHOqbcQObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8JRu3DVAW1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dah: Option<ObjectOfUnorderedSetOfStringBA0Rq1IJjh9Zs8ELUnorderedSetOfStringBA0Rq1IJjh9Zs8ELBE2T5R7P>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<ObjectOfObjectOfString2AHOqbcQString2AHOqbcQRRuuXHRJStringBA0Rq1IJStringV0UwQeVuStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJObjectOfObjectOfString2AHOqbcQStringBA0Rq1IJWyeefrDWStringBA0Rq1IJ3Lynca8JString2AHOqbcQStringBA0Rq1IJStringBA0Rq1IJStringBA0Rq1IJStringDoaGddGAStringBA0Rq1IJPuwa0B7M>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_set: Option<ObjectOfUnorderedSetOfObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy232V8C0Y7PObjectOfString2AHOqbcQAnyZc4BnYi3String2AHOqbcQStringBA0Rq1IJIAmgVy23KDJSbC9F>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectGbKRBxC9 {
    ObjectGbKRBxC9AsObject(ObjectGbKRBxC9AsObject),
}
/// ArrayKy5X9JLGAsArray
///
/// # Example
///
/// `[object Object]`
///
pub type ArrayKy5X9JLGAsArray = (ObjectGbKRBxC9);
/// ObjectTwaQ0BR1AsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct SyncState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<StringV0UwQeVu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<StringV0UwQeVu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_hash: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_height: Option<u64>,
}
/// ObjectGE2SwJm3AsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectGE2SwJm3AsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<StringDoaGddGA>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub _type: Option<String2AHOqbcQ>,
}
pub type NumberHo1ClIqD = f64;
/// ObjectEyXSJtLSAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectEyXSJtLSAsObject {
    #[serde(rename = "RateIn", skip_serializing_if = "Option::is_none")]
    pub rate_in: Option<NumberHo1ClIqD>,
    #[serde(rename = "RateOut", skip_serializing_if = "Option::is_none")]
    pub rate_out: Option<NumberHo1ClIqD>,
    #[serde(rename = "TotalIn", skip_serializing_if = "Option::is_none")]
    pub total_in: Option<String2AHOqbcQ>,
    #[serde(rename = "TotalOut", skip_serializing_if = "Option::is_none")]
    pub total_out: Option<String2AHOqbcQ>,
}
/// IntegerVBWLmfONAsInteger
///
/// # Example
///
/// `1`
///
pub type IntegerVBWLmfONAsInteger = i64;
/// ArrayEem6MnzqAsArray
///
/// # Example
///
/// `CovLVG4fQcqUS6DmoMxAwVJGNW6PMzfwTG6BHW9NH9TLGHcbRfvPVc3JVhnufK3HTzStoTo`
///
pub type ArrayEem6MnzqAsArray = (StringOfElfKS1);
/// IntegerLSS5Yq6OAsInteger
///
/// # Example
///
/// `2`
///
pub type IntegerLSS5Yq6OAsInteger = i64;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQK1NgycVD
{
    #[serde(rename = "Memory", skip_serializing_if = "Option::is_none")]
    pub memory: Option<String2AHOqbcQ>,
    #[serde(rename = "NumConnsInbound", skip_serializing_if = "Option::is_none")]
    pub num_conns_inbound: Option<String2AHOqbcQ>,
    #[serde(rename = "NumConnsOutbound", skip_serializing_if = "Option::is_none")]
    pub num_conns_outbound: Option<String2AHOqbcQ>,
    #[serde(rename = "NumFD", skip_serializing_if = "Option::is_none")]
    pub num_fd: Option<String2AHOqbcQ>,
    #[serde(rename = "NumStreamsInbound", skip_serializing_if = "Option::is_none")]
    pub num_streams_inbound: Option<String2AHOqbcQ>,
    #[serde(rename = "NumStreamsOutbound", skip_serializing_if = "Option::is_none")]
    pub num_streams_outbound: Option<String2AHOqbcQ>,
}
pub type ObjectMPaq4Myi = String;
/// ObjectKwkulJGDAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectKwkulJGDAsObject {
    #[serde(rename = "Peers", skip_serializing_if = "Option::is_none")]
    pub peers: Option<ObjectMPaq4Myi>,
    #[serde(rename = "Protocols", skip_serializing_if = "Option::is_none")]
    pub protocols: Option<ObjectMPaq4Myi>,
    #[serde(rename = "Services", skip_serializing_if = "Option::is_none")]
    pub services: Option<ObjectMPaq4Myi>,
    #[serde(rename = "System", skip_serializing_if = "Option::is_none")]
    pub system: Option<ObjectOfString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQK1NgycVD>,
    #[serde(rename = "Transient", skip_serializing_if = "Option::is_none")]
    pub transient: Option<ObjectOfString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQString2AHOqbcQK1NgycVD>,
}
/// ObjectGJ2SCEHXAsObject
///
/// # Example
///
/// `[object Object]`
///
pub type ObjectGJ2SCEHXAsObject = String;
/// String2IbrpYpSAsString
///
/// # Example
///
/// `Ynl0ZSBhcnJheQ==`
///
pub type String2IbrpYpSAsString = String;
pub type ObjectM9Mo69S8 = String;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct Object4614IS9JAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof: Option<ObjectM9Mo69S8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<UnorderedSetOfStringBA0Rq1IJjh9Zs8EL>,
}

pub type Array88Kxwnl9 = Vec<Object4614IS9JAsObject>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Object4614IS9J {
    Object4614IS9JAsObject(Object4614IS9JAsObject),
}
/// ArrayLvGYXONEAsArray
///
/// # Example
///
/// `[object Object]`
///
pub type ArrayLvGYXONEAsArray = (Object4614IS9J);
/// ObjectYNJBabZPAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectYNJBabZPAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<ObjectM9Mo69S8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denom: Option<StringDoaGddGA>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<BooleanVyG3AETh>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<StringBA0Rq1IJ>,
}
pub type UnorderedSetOfObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR4GEEMzeG =
    Vec<ObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR4GEEMzeGAxKOTClL
{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes:
        Option<UnorderedSetOfObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR4GEEMzeG>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub _type: Option<StringDoaGddGA>,
}
pub type UnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR4GEEMzeGAxKOTClL1DSpgDol = Vec<ObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR4GEEMzeGAxKOTClL>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<StringDoaGddGA>,
}
pub type UnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46 =
    Vec<ObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAe>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp
{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<UnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub _type: Option<StringDoaGddGA>,
}
pub type UnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp0ZFDmlkf = Vec<ObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfString2AHOqbcQStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp0ZFDmlkfZ8XFh1Ek {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<UnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp0ZFDmlkf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_index: Option<String2AHOqbcQ>,
}
pub type UnorderedSetOfObjectOfString2AHOqbcQStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp0ZFDmlkfZ8XFh1EkciQyORJM = Vec<ObjectOfString2AHOqbcQStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp0ZFDmlkfZ8XFh1Ek>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringBA0Rq1IJStringDoaGddGA0BUJF0L3 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_url: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<StringBA0Rq1IJ>,
}
/// ObjectYKO2DXRYAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct TxResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codespace: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<UnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringBA0Rq1IJStringBA0Rq1IJBooleanVyG3AEThINeHvcCR4GEEMzeGAxKOTClL1DSpgDol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_used: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_wanted: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<UnorderedSetOfObjectOfString2AHOqbcQStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAUnorderedSetOfObjectOfStringDoaGddGAStringDoaGddGAXUt3GaAeLSHexv46Ql9MlHsp0ZFDmlkfZ8XFh1EkciQyORJM>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_log: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx: Option<ObjectOfStringBA0Rq1IJStringDoaGddGA0BUJF0L3>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txhash: Option<StringDoaGddGA>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringDoaGddGAObjectM9Mo69S8LtMqrUfa {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<ObjectM9Mo69S8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denom: Option<StringDoaGddGA>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringDoaGddGAObjectM9Mo69S8StringDoaGddGADuNYclxU {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegator_address: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<ObjectM9Mo69S8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_address: Option<StringDoaGddGA>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfObjectOfStringDoaGddGAObjectM9Mo69S8StringDoaGddGADuNYclxUObjectOfStringDoaGddGAObjectM9Mo69S8LtMqrUfaV5ITXG39
{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<ObjectOfStringDoaGddGAObjectM9Mo69S8LtMqrUfa>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegation: Option<ObjectOfStringDoaGddGAObjectM9Mo69S8StringDoaGddGADuNYclxU>,
}
/// ObjectYlejlgYNAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectYlejlgYNAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegation_response: Option<ObjectOfObjectOfStringDoaGddGAObjectM9Mo69S8StringDoaGddGADuNYclxUObjectOfStringDoaGddGAObjectM9Mo69S8LtMqrUfaV5ITXG39>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfString2AHOqbcQStringBA0Rq1IJECQBJ1Wt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_key: Option<StringBA0Rq1IJ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<String2AHOqbcQ>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3Tf {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_time: Option<StringV0UwQeVu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_height: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_balance: Option<ObjectM9Mo69S8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares_dst: Option<ObjectM9Mo69S8>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdn
{
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<ObjectM9Mo69S8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redelegation_entry:
        Option<ObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3Tf>,
}
pub type UnorderedSetOfObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdnGaGRkzl1 = Vec<ObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdn>;
pub type UnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0 =
    Vec<ObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3Tf>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringDoaGddGAStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0StringDoaGddGA71JdgukU {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegator_address: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<UnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_dst_address: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_src_address: Option<StringDoaGddGA>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfObjectOfStringDoaGddGAStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0StringDoaGddGA71JdgukUUnorderedSetOfObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdnGaGRkzl1RLfadiYB {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<UnorderedSetOfObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdnGaGRkzl1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redelegation: Option<ObjectOfStringDoaGddGAStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0StringDoaGddGA71JdgukU>,
}
pub type UnorderedSetOfObjectOfObjectOfStringDoaGddGAStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0StringDoaGddGA71JdgukUUnorderedSetOfObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdnGaGRkzl1RLfadiYBpV1UEM4N = Vec<ObjectOfObjectOfStringDoaGddGAStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0StringDoaGddGA71JdgukUUnorderedSetOfObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdnGaGRkzl1RLfadiYB>;
/// Object0SE0XlKCAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct Object0SE0XlKCAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<ObjectOfString2AHOqbcQStringBA0Rq1IJECQBJ1Wt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redelegation_responses: Option<UnorderedSetOfObjectOfObjectOfStringDoaGddGAStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfvnCyCAA0StringDoaGddGA71JdgukUUnorderedSetOfObjectOfObjectOfObjectM9Mo69S8ObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuQaHMa3TfObjectM9Mo69S8ISwnaEdnGaGRkzl1RLfadiYBpV1UEM4N>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuObjectM9Mo69S84X3MQltY {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<ObjectM9Mo69S8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_time: Option<StringV0UwQeVu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_height: Option<String2AHOqbcQ>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_balance: Option<ObjectM9Mo69S8>,
}
pub type UnorderedSetOfObjectOfObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuObjectM9Mo69S84X3MQltY4VleNL3V =
    Vec<ObjectOfObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuObjectM9Mo69S84X3MQltY>;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct ObjectOfStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuObjectM9Mo69S84X3MQltY4VleNL3VStringDoaGddGABMBOnfsr {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegator_address: Option<StringDoaGddGA>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<UnorderedSetOfObjectOfObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuObjectM9Mo69S84X3MQltY4VleNL3V>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator_address: Option<StringDoaGddGA>,
}
/// Object7ZHqSpQgAsObject
///
/// # Example
///
/// `[object Object]`
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Builder, Default)]
#[builder(setter(strip_option), default)]
#[serde(default)]
pub struct Object7ZHqSpQgAsObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unbond: Option<ObjectOfStringDoaGddGAUnorderedSetOfObjectOfObjectM9Mo69S8String2AHOqbcQStringV0UwQeVuObjectM9Mo69S84X3MQltY4VleNL3VStringDoaGddGABMBOnfsr>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringBvkcVWUg {
    StringBvkcVWUgAsString(StringBvkcVWUgAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringNayfGy6P {
    StringNayfGy6PAsString(StringNayfGy6PAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ArrayP2Psh38G {
    ArrayP2Psh38GAsArray(ArrayP2Psh38GAsArray),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ArrayXVPF8Nrg {
    ArrayXVPF8NrgAsArray(ArrayXVPF8NrgAsArray),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ArrayI8QwXWBj {
    ArrayI8QwXWBjAsArray(ArrayI8QwXWBjAsArray),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectOq6Lbs0X {
    ObjectOq6Lbs0XAsObject(ObjectOq6Lbs0XAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringRulx2YlQ {
    StringRulx2YlQAsString(StringRulx2YlQAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringQrjJ8DPh {
    StringQrjJ8DPhAsString(StringQrjJ8DPhAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectIIoInpd7 {
    ObjectIIoInpd7AsObject(ObjectIIoInpd7AsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Array8BVxQ2Xk {
    Array8BVxQ2XkAsArray(Array8BVxQ2XkAsArray),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringEprQ2TNC {
    StringEprQ2TNCAsString(StringEprQ2TNCAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum String9UfShva3 {
    String9UfShva3AsString(String9UfShva3AsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringLnSWZgcx {
    StringLnSWZgcxAsString(StringLnSWZgcxAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectHWS0RFGb {
    ObjectHWS0RFGbAsObject(ObjectHWS0RFGbAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectQ6MEBEIP {
    ObjectQ6MEBEIPAsObject(ObjectQ6MEBEIPAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringI8Xo6Awh {
    StringI8Xo6AwhAsString(StringI8Xo6AwhAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectOPxuZea0 {
    ObjectOPxuZea0AsObject(ObjectOPxuZea0AsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum StringPZrCQoJg {
    StringPZrCQoJgAsString(StringPZrCQoJgAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectFTcB5RWo {
    ObjectFTcB5RWoAsObject(ObjectFTcB5RWoAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum BooleanIYY4Gv1X {
    BooleanIYY4Gv1XAsBoolean(BooleanIYY4Gv1XAsBoolean),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectSVDfQMta {
    ObjectSVDfQMtaAsObject(ObjectSVDfQMtaAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum NullIUT54VvM {
    NullIUT54VvMAsNull(NullIUT54VvMAsNull),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ArrayNfMA4Xz1 {
    ArrayNfMA4Xz1AsArray(ArrayNfMA4Xz1AsArray),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum TypeUnsupportedByJSONSchema {
    TypeUnsupportedByJSONSchemaAsObject(TypeUnsupportedByJSONSchemaAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ArrayKy5X9JLG {
    ArrayKy5X9JLGAsArray(ArrayKy5X9JLGAsArray),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectGE2SwJm3 {
    ObjectGE2SwJm3AsObject(ObjectGE2SwJm3AsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectEyXSJtLS {
    ObjectEyXSJtLSAsObject(ObjectEyXSJtLSAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum IntegerVBWLmfON {
    IntegerVBWLmfONAsInteger(IntegerVBWLmfONAsInteger),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ArrayEem6Mnzq {
    ArrayEem6MnzqAsArray(ArrayEem6MnzqAsArray),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum IntegerLSS5Yq6O {
    IntegerLSS5Yq6OAsInteger(IntegerLSS5Yq6OAsInteger),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectKwkulJGD {
    ObjectKwkulJGDAsObject(ObjectKwkulJGDAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectGJ2SCEHX {
    ObjectGJ2SCEHXAsObject(ObjectGJ2SCEHXAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum String2IbrpYpS {
    String2IbrpYpSAsString(String2IbrpYpSAsString),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ArrayLvGYXONE {
    ArrayLvGYXONEAsArray(ArrayLvGYXONEAsArray),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectYNJBabZP {
    ObjectYNJBabZPAsObject(ObjectYNJBabZPAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectYKO2DXRY {
    ObjectYKO2DXRYAsObject(TxResponse),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum ObjectYlejlgYN {
    ObjectYlejlgYNAsObject(ObjectYlejlgYNAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Object0SE0XlKC {
    Object0SE0XlKCAsObject(Object0SE0XlKCAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum Object7ZHqSpQg {
    Object7ZHqSpQgAsObject(Object7ZHqSpQgAsObject),
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum AnyOfInteger7BU60JXWStringBvkcVWUgStringNayfGy6PInteger7BU60JXWArrayP2Psh38GInteger7BU60JXWStringBvkcVWUgStringNayfGy6PInteger7BU60JXWStringBvkcVWUgArrayXVPF8NrgStringNayfGy6PArrayI8QwXWBjObjectOq6Lbs0XStringRulx2YlQStringRulx2YlQStringQrjJ8DPhInteger7BU60JXWObjectIIoInpd7Integer7BU60JXWInteger7BU60JXWArray8BVxQ2XkStringEprQ2TNCStringEprQ2TNCStringEprQ2TNCString9UfShva3StringLnSWZgcxString9UfShva3String9UfShva3ObjectHWS0RFGbString9UfShva3String9UfShva3StringEprQ2TNCString9UfShva3String9UfShva3StringEprQ2TNCStringEprQ2TNCString9UfShva3String9UfShva3StringEprQ2TNCObjectIIoInpd7ObjectIIoInpd7Integer7BU60JXWInteger7BU60JXWObjectIIoInpd7StringBvkcVWUgObjectIIoInpd7ObjectQ6MEBEIPStringI8Xo6AwhStringI8Xo6AwhObjectOPxuZea0ObjectOPxuZea0Integer7BU60JXWStringI8Xo6AwhObjectOPxuZea0ObjectOPxuZea0ObjectOPxuZea0Integer7BU60JXWStringI8Xo6AwhObjectOPxuZea0ObjectOPxuZea0Integer7BU60JXWStringI8Xo6AwhStringI8Xo6AwhStringI8Xo6AwhStringI8Xo6AwhObjectOPxuZea0Integer7BU60JXWArrayI8QwXWBjStringNayfGy6PStringPZrCQoJgObjectOPxuZea0ObjectOPxuZea0Integer7BU60JXWStringI8Xo6AwhObjectOPxuZea0ObjectOPxuZea0Integer7BU60JXWObjectFTcB5RWoArrayI8QwXWBjArrayXVPF8NrgBooleanIYY4Gv1XInteger7BU60JXWObjectSVDfQMtaNullIUT54VvMArrayNfMA4Xz1TypeUnsupportedByJSONSchemaObjectIIoInpd7ObjectIIoInpd7ArrayKy5X9JLGObjectIIoInpd7ObjectIIoInpd7TypeUnsupportedByJSONSchemaObjectTwaQ0BR1NullIUT54VvMObjectIIoInpd7StringEprQ2TNCArray8BVxQ2XkObjectGE2SwJm3NullIUT54VvMObjectEyXSJtLSObjectEyXSJtLSObjectEyXSJtLSNullIUT54VvMNullIUT54VvMNullIUT54VvMIntegerVBWLmfONObjectHWS0RFGbBooleanIYY4Gv1XArrayEem6MnzqIntegerLSS5Yq6OObjectHWS0RFGbArrayEem6MnzqNullIUT54VvMArrayEem6MnzqObjectKwkulJGDNullIUT54VvMBooleanIYY4Gv1XObjectGJ2SCEHXString2IbrpYpSArrayLvGYXONENullIUT54VvMObjectQ6MEBEIPObjectYNJBabZPObjectYNJBabZPObjectYKO2DXRYObjectYKO2DXRYObjectYKO2DXRYBooleanIYY4Gv1XObjectYlejlgYNObject0SE0XlKCObject7ZHqSpQgObjectYKO2DXRYObjectYKO2DXRYObjectYKO2DXRYObjectYKO2DXRY
{
    StringBvkcVWUg(StringBvkcVWUg),
    StringNayfGy6P(StringNayfGy6P),
    ArrayP2Psh38G(ArrayP2Psh38G),
    ArrayXVPF8Nrg(ArrayXVPF8Nrg),
    ArrayI8QwXWBj(ArrayI8QwXWBj),
    ObjectOq6Lbs0X(ObjectOq6Lbs0X),
    StringRulx2YlQ(StringRulx2YlQ),
    StringQrjJ8DPh(StringQrjJ8DPh),
    ObjectIIoInpd7(ObjectIIoInpd7),
    Array8BVxQ2Xk(Array8BVxQ2Xk),
    StringEprQ2TNC(StringEprQ2TNC),
    String9UfShva3(String9UfShva3),
    StringLnSWZgcx(StringLnSWZgcx),
    ObjectHWS0RFGb(ObjectHWS0RFGb),
    ObjectQ6MEBEIP(ObjectQ6MEBEIP),
    StringI8Xo6Awh(StringI8Xo6Awh),
    ObjectOPxuZea0(ObjectOPxuZea0),
    StringPZrCQoJg(StringPZrCQoJg),
    ObjectFTcB5RWo(ObjectFTcB5RWo),
    BooleanIYY4Gv1X(BooleanIYY4Gv1X),
    ObjectSVDfQMta(ObjectSVDfQMta),
    NullIUT54VvM(NullIUT54VvM),
    ArrayNfMA4Xz1(ArrayNfMA4Xz1),
    TypeUnsupportedByJSONSchema(TypeUnsupportedByJSONSchema),
    ArrayKy5X9JLG(ArrayKy5X9JLG),
    ObjectGE2SwJm3(ObjectGE2SwJm3),
    ObjectEyXSJtLS(ObjectEyXSJtLS),
    IntegerVBWLmfON(IntegerVBWLmfON),
    ArrayEem6Mnzq(ArrayEem6Mnzq),
    IntegerLSS5Yq6O(IntegerLSS5Yq6O),
    ObjectKwkulJGD(ObjectKwkulJGD),
    ObjectGJ2SCEHX(ObjectGJ2SCEHX),
    String2IbrpYpS(String2IbrpYpS),
    ArrayLvGYXONE(ArrayLvGYXONE),
    ObjectYNJBabZP(ObjectYNJBabZP),
    ObjectYKO2DXRY(ObjectYKO2DXRY),
    ObjectYlejlgYN(ObjectYlejlgYN),
    Object0SE0XlKC(Object0SE0XlKC),
    Object7ZHqSpQg(Object7ZHqSpQg),
}
#[derive(Clone)]
pub struct CelestiaNodeAPI<T> {
    transport: Box<T>,
}

impl<T: jsonrpsee::core::client::ClientT + Send + Sync> CelestiaNodeAPI<T>
where
    T: Send + Sync + 'static,
{
    pub fn new(transport: T) -> Self {
        CelestiaNodeAPI {
            transport: Box::new(transport),
        }
    }

    pub fn BlobGet<'a>(
        &'a self,
        height: i64,
        namespace: String,
        commitment: String,
    ) -> Pin<Box<dyn Future<Output = Result<Blob, Error>> + Send + 'a>> {
        let method = "blob.Get";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(height).unwrap();
        params.insert(namespace).unwrap();
        params.insert(commitment).unwrap();
        self.transport.request(method, params)
    }

    pub fn BlobGetAll<'a>(
        &'a self,
        height: u64,
        namespaces: Vec<String>,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Blob>, Error>> + Send + 'a>> {
        let method = "blob.GetAll";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(height).unwrap();
        params.insert(namespaces).unwrap();
        self.transport.request(method, params)
    }

    pub fn BlobGetProof<'a>(
        &'a self,
        height: Integer7BU60JXWAsInteger,
        namespace: StringBvkcVWUg,
        commitment: StringNayfGy6P,
    ) -> Pin<Box<dyn Future<Output = Result<ArrayYTcMIOLC, Error>> + Send + 'a>> {
        let method = "blob.GetProof";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(height).unwrap();
        params.insert(namespace).unwrap();
        params.insert(commitment).unwrap();
        self.transport.request(method, params)
    }

    pub fn BlobIncluded<'a>(
        &'a self,
        height: Integer7BU60JXWAsInteger,
        namespace: StringBvkcVWUg,
        proof: ArrayYTcMIOLC,
        commitment: StringNayfGy6P,
    ) -> Pin<Box<dyn Future<Output = Result<BooleanIYY4Gv1X, Error>> + Send + 'a>> {
        let method = "blob.Included";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(height).unwrap();
        params.insert(namespace).unwrap();
        params.insert(proof).unwrap();
        params.insert(commitment).unwrap();
        self.transport.request(method, params)
    }

    pub fn BlobSubmit<'a>(
        &'a self,
        blobs: &[Blob],
        options: ObjectOq6Lbs0X,
    ) -> Pin<Box<dyn Future<Output = Result<i64, Error>> + Send + 'a>> {
        let method = "blob.Submit";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(blobs).unwrap();
        params.insert(options).unwrap();
        self.transport.request(method, params)
    }

    pub fn DaserSamplingStats<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectSVDfQMta, Error>> + Send + 'a>> {
        let method = "daser.SamplingStats";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn DaserWaitCatchUp<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "daser.WaitCatchUp";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn FraudGet<'a>(
        &'a self,
        proofType: StringRulx2YlQ,
    ) -> Pin<Box<dyn Future<Output = Result<ArrayWenhckXh, Error>> + Send + 'a>> {
        let method = "fraud.Get";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(proofType).unwrap();
        self.transport.request(method, params)
    }

    pub fn FraudSubscribe<'a>(
        &'a self,
        proofType: StringRulx2YlQ,
    ) -> Pin<Box<dyn Future<Output = Result<TypeUnsupportedByJSONSchema, Error>> + Send + 'a>> {
        let method = "fraud.Subscribe";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(proofType).unwrap();
        self.transport.request(method, params)
    }

    pub fn HeaderGetByHash<'a>(
        &'a self,
        hash: StringQrjJ8DPh,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectIIoInpd7, Error>> + Send + 'a>> {
        let method = "header.GetByHash";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(hash).unwrap();
        self.transport.request(method, params)
    }

    pub fn HeaderGetByHeight<'a>(
        &'a self,
        u: u64,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectIIoInpd7AsObject, Error>> + Send + 'a>> {
        let method = "header.GetByHeight";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(u).unwrap();
        self.transport.request(method, params)
    }

    pub fn HeaderGetRangeByHeight<'a>(
        &'a self,
        from: ObjectIIoInpd7,
        to: Integer7BU60JXWAsInteger,
    ) -> Pin<Box<dyn Future<Output = Result<ArrayHQjRkX0C, Error>> + Send + 'a>> {
        let method = "header.GetRangeByHeight";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(from).unwrap();
        params.insert(to).unwrap();
        self.transport.request(method, params)
    }

    pub fn HeaderLocalHead<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectIIoInpd7, Error>> + Send + 'a>> {
        let method = "header.LocalHead";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn HeaderNetworkHead<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectIIoInpd7, Error>> + Send + 'a>> {
        let method = "header.NetworkHead";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn HeaderSubscribe<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<TypeUnsupportedByJSONSchema, Error>> + Send + 'a>> {
        let method = "header.Subscribe";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn HeaderSyncState<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<SyncState, Error>> + Send + 'a>> {
        let method = "header.SyncState";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn HeaderSyncWait<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "header.SyncWait";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn HeaderWaitForHeight<'a>(
        &'a self,
        u: u64,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectIIoInpd7AsObject, Error>> + Send + 'a>> {
        let method = "header.WaitForHeight";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(u).unwrap();
        self.transport.request(method, params)
    }

    pub fn NodeAuthNew<'a>(
        &'a self,
        perms: ArrayGnKkLVm4,
    ) -> Pin<Box<dyn Future<Output = Result<StringEprQ2TNC, Error>> + Send + 'a>> {
        let method = "node.AuthNew";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(perms).unwrap();
        self.transport.request(method, params)
    }

    pub fn NodeAuthVerify<'a>(
        &'a self,
        token: StringEprQ2TNC,
    ) -> Pin<Box<dyn Future<Output = Result<ArrayGnKkLVm4, Error>> + Send + 'a>> {
        let method = "node.AuthVerify";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(token).unwrap();
        self.transport.request(method, params)
    }

    pub fn NodeInfo<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectGE2SwJm3, Error>> + Send + 'a>> {
        let method = "node.Info";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn NodeLogLevelSet<'a>(
        &'a self,
        name: StringEprQ2TNC,
        level: StringEprQ2TNC,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "node.LogLevelSet";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(name).unwrap();
        params.insert(level).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PBandwidthForPeer<'a>(
        &'a self,
        id: String9UfShva3,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectEyXSJtLS, Error>> + Send + 'a>> {
        let method = "p2p.BandwidthForPeer";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(id).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PBandwidthForProtocol<'a>(
        &'a self,
        proto: StringLnSWZgcx,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectEyXSJtLS, Error>> + Send + 'a>> {
        let method = "p2p.BandwidthForProtocol";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(proto).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PBandwidthStats<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectEyXSJtLS, Error>> + Send + 'a>> {
        let method = "p2p.BandwidthStats";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn P2PBlockPeer<'a>(
        &'a self,
        p: String9UfShva3,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "p2p.BlockPeer";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(p).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PClosePeer<'a>(
        &'a self,
        id: String9UfShva3,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "p2p.ClosePeer";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(id).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PConnect<'a>(
        &'a self,
        pi: ObjectHWS0RFGb,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "p2p.Connect";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(pi).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PConnectedness<'a>(
        &'a self,
        id: String9UfShva3,
    ) -> Pin<Box<dyn Future<Output = Result<IntegerVBWLmfON, Error>> + Send + 'a>> {
        let method = "p2p.Connectedness";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(id).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PInfo<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectHWS0RFGb, Error>> + Send + 'a>> {
        let method = "p2p.Info";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn P2PIsProtected<'a>(
        &'a self,
        id: String9UfShva3,
        tag: StringEprQ2TNC,
    ) -> Pin<Box<dyn Future<Output = Result<BooleanIYY4Gv1X, Error>> + Send + 'a>> {
        let method = "p2p.IsProtected";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(id).unwrap();
        params.insert(tag).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PListBlockedPeers<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ArrayRFsCAiJu, Error>> + Send + 'a>> {
        let method = "p2p.ListBlockedPeers";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn P2PNATStatus<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<IntegerLSS5Yq6O, Error>> + Send + 'a>> {
        let method = "p2p.NATStatus";
        let params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn P2PPeerInfo<'a>(
        &'a self,
        id: String9UfShva3,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectHWS0RFGb, Error>> + Send + 'a>> {
        let method = "p2p.PeerInfo";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(id).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PPeers<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ArrayRFsCAiJu, Error>> + Send + 'a>> {
        let method = "p2p.Peers";
        let params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn P2PProtect<'a>(
        &'a self,
        id: String9UfShva3,
        tag: StringEprQ2TNC,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "p2p.Protect";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(id).unwrap();
        params.insert(tag).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PPubSubPeers<'a>(
        &'a self,
        topic: StringEprQ2TNC,
    ) -> Pin<Box<dyn Future<Output = Result<ArrayRFsCAiJu, Error>> + Send + 'a>> {
        let method = "p2p.PubSubPeers";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(topic).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PResourceState<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectKwkulJGD, Error>> + Send + 'a>> {
        let method = "p2p.ResourceState";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn P2PUnblockPeer<'a>(
        &'a self,
        p: String9UfShva3,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "p2p.UnblockPeer";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(p).unwrap();
        self.transport.request(method, params)
    }

    pub fn P2PUnprotect<'a>(
        &'a self,
        id: String9UfShva3,
        tag: StringEprQ2TNC,
    ) -> Pin<Box<dyn Future<Output = Result<BooleanIYY4Gv1X, Error>> + Send + 'a>> {
        let method = "p2p.Unprotect";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(id).unwrap();
        params.insert(tag).unwrap();
        self.transport.request(method, params)
    }

    pub fn ShareGetEDS<'a>(
        &'a self,
        header: ObjectIIoInpd7,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectGJ2SCEHX, Error>> + Send + 'a>> {
        let method = "share.GetEDS";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(header).unwrap();
        self.transport.request(method, params)
    }

    pub fn ShareGetShare<'a>(
        &'a self,
        header: ObjectIIoInpd7,
        row: Integer7BU60JXWAsInteger,
        col: Integer7BU60JXWAsInteger,
    ) -> Pin<Box<dyn Future<Output = Result<String2IbrpYpS, Error>> + Send + 'a>> {
        let method = "share.GetShare";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(header).unwrap();
        params.insert(row).unwrap();
        params.insert(col).unwrap();
        self.transport.request(method, params)
    }

    pub fn ShareGetSharesByNamespace<'a>(
        &'a self,
        header: ObjectIIoInpd7,
        namespace: StringBvkcVWUg,
    ) -> Pin<Box<dyn Future<Output = Result<Array88Kxwnl9, Error>> + Send + 'a>> {
        let method = "share.GetSharesByNamespace";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(header).unwrap();
        params.insert(namespace).unwrap();
        self.transport.request(method, params)
    }

    pub fn ShareSharesAvailable<'a>(
        &'a self,
        header: ObjectIIoInpd7,
    ) -> Pin<Box<dyn Future<Output = Result<NullIUT54VvM, Error>> + Send + 'a>> {
        let method = "share.SharesAvailable";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(header).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateAccountAddress<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectQ6MEBEIP, Error>> + Send + 'a>> {
        let method = "state.AccountAddress";
        let params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn StateBalance<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYNJBabZP, Error>> + Send + 'a>> {
        let method = "state.Balance";
        let params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn StateBalanceForAddress<'a>(
        &'a self,
        addr: ObjectQ6MEBEIP,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYNJBabZP, Error>> + Send + 'a>> {
        let method = "state.BalanceForAddress";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(addr).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateBeginRedelegate<'a>(
        &'a self,
        srcValAddr: StringI8Xo6Awh,
        dstValAddr: StringI8Xo6Awh,
        amount: ObjectOPxuZea0,
        fee: ObjectOPxuZea0,
        gasLim: Integer7BU60JXWAsInteger,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYKO2DXRY, Error>> + Send + 'a>> {
        let method = "state.BeginRedelegate";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(srcValAddr).unwrap();
        params.insert(dstValAddr).unwrap();
        params.insert(amount).unwrap();
        params.insert(fee).unwrap();
        params.insert(gasLim).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateCancelUnbondingDelegation<'a>(
        &'a self,
        valAddr: StringI8Xo6Awh,
        amount: ObjectOPxuZea0,
        height: ObjectOPxuZea0,
        fee: ObjectOPxuZea0,
        gasLim: Integer7BU60JXWAsInteger,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYKO2DXRY, Error>> + Send + 'a>> {
        let method = "state.CancelUnbondingDelegation";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(valAddr).unwrap();
        params.insert(amount).unwrap();
        params.insert(height).unwrap();
        params.insert(fee).unwrap();
        params.insert(gasLim).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateDelegate<'a>(
        &'a self,
        delAddr: StringI8Xo6Awh,
        amount: ObjectOPxuZea0,
        fee: ObjectOPxuZea0,
        gasLim: Integer7BU60JXWAsInteger,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYKO2DXRY, Error>> + Send + 'a>> {
        let method = "state.Delegate";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(delAddr).unwrap();
        params.insert(amount).unwrap();
        params.insert(fee).unwrap();
        params.insert(gasLim).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateIsStopped<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<BooleanIYY4Gv1X, Error>> + Send + 'a>> {
        let method = "state.IsStopped";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        self.transport.request(method, params)
    }

    pub fn StateQueryDelegation<'a>(
        &'a self,
        valAddr: StringI8Xo6Awh,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYlejlgYN, Error>> + Send + 'a>> {
        let method = "state.QueryDelegation";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(valAddr).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateQueryRedelegations<'a>(
        &'a self,
        srcValAddr: StringI8Xo6Awh,
        dstValAddr: StringI8Xo6Awh,
    ) -> Pin<Box<dyn Future<Output = Result<Object0SE0XlKC, Error>> + Send + 'a>> {
        let method = "state.QueryRedelegations";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(srcValAddr).unwrap();
        params.insert(dstValAddr).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateQueryUnbonding<'a>(
        &'a self,
        valAddr: StringI8Xo6Awh,
    ) -> Pin<Box<dyn Future<Output = Result<Object7ZHqSpQg, Error>> + Send + 'a>> {
        let method = "state.QueryUnbonding";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(valAddr).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateSubmitPayForBlob<'a>(
        &'a self,
        fee: ObjectOPxuZea0,
        gasLim: Integer7BU60JXWAsInteger,
        blobs: Vec<Blob>,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYKO2DXRY, Error>> + Send + 'a>> {
        let method = "state.SubmitPayForBlob";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(fee).unwrap();
        params.insert(gasLim).unwrap();
        params.insert(blobs).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateSubmitTx<'a>(
        &'a self,
        tx: String,
    ) -> Pin<Box<dyn Future<Output = Result<TxResponse, Error>> + Send + 'a>> {
        let method = "state.SubmitTx";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(tx).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateTransfer<'a>(
        &'a self,
        to: StringPZrCQoJg,
        amount: ObjectOPxuZea0,
        fee: ObjectOPxuZea0,
        gasLimit: Integer7BU60JXWAsInteger,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYKO2DXRY, Error>> + Send + 'a>> {
        let method = "state.Transfer";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(to).unwrap();
        params.insert(amount).unwrap();
        params.insert(fee).unwrap();
        params.insert(gasLimit).unwrap();
        self.transport.request(method, params)
    }

    pub fn StateUndelegate<'a>(
        &'a self,
        del_addr: StringI8Xo6Awh,
        amount: ObjectOPxuZea0,
        fee: ObjectOPxuZea0,
        gas_lim: Integer7BU60JXWAsInteger,
    ) -> Pin<Box<dyn Future<Output = Result<ObjectYKO2DXRY, Error>> + Send + 'a>> {
        let method = "state.Undelegate";
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert(del_addr).unwrap();
        params.insert(amount).unwrap();
        params.insert(fee).unwrap();
        params.insert(gas_lim).unwrap();
        self.transport.request(method, params)
    }
}