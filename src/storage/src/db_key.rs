//
// db_key.rs
// Copyright (C) 2022 db3.network Author imotai <codego.me@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use db3_crypto::db3_address::{DB3Address, DB3_ADDRESS_LENGTH};
use db3_error::{DB3Error, Result};
/// account_address + _NS_ + ns
pub struct DbKey<'a>(pub DB3Address, pub &'a [u8]);

const DATABASE: &str = "_DB_";
impl<'a> DbKey<'a> {
    ///
    /// encode the key
    ///
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut encoded_key = self.0.as_ref().to_vec();
        encoded_key.extend_from_slice(DATABASE.as_bytes());
        encoded_key.extend_from_slice(self.1);
        Ok(encoded_key)
    }

    ///
    /// decode the key
    ///
    pub fn decode(data: &'a [u8]) -> Result<Self> {
        const MIN_KEY_TOTAL_LEN: usize = DB3_ADDRESS_LENGTH + DATABASE.len();
        if data.len() <= MIN_KEY_TOTAL_LEN {
            return Err(DB3Error::KeyCodecError(
                "the length of data is invalid".to_string(),
            ));
        }
        let key_start_offset = MIN_KEY_TOTAL_LEN;
        let data_slice: &[u8; DB3_ADDRESS_LENGTH] = &data[..DB3_ADDRESS_LENGTH]
            .try_into()
            .map_err(|e| DB3Error::KeyCodecError(format!("{e}")))?;
        let addr = DB3Address::from(data_slice);
        Ok(Self(addr, &data[key_start_offset..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use db3_crypto::key_derive;
    use db3_crypto::signature_scheme::SignatureScheme;

    fn gen_address() -> DB3Address {
        let seed: [u8; 32] = [0; 32];
        let (address, _) =
            key_derive::derive_key_pair_from_path(&seed, None, &SignatureScheme::ED25519).unwrap();
        address
    }

    #[test]
    fn it_key_serde() {
        let addr = gen_address();
        let ns: &str = "ns1";
        let key = DbKey(addr, ns.as_bytes());
        let key_encoded = key.encode();
        assert!(key_encoded.is_ok());
        let key_decoded = DbKey::decode(key_encoded.as_ref().unwrap());
        assert!(key_decoded.is_ok());
        let key2 = key_decoded.unwrap();
        assert!(key2.0 == addr);
        assert_eq!(key2.1, ns.as_bytes().to_vec());
    }

    #[test]
    fn it_key_serde_cmp() -> Result<()> {
        let addr = gen_address();
        let ns: &str = "ns1";
        let key = DbKey(addr, ns.as_bytes());
        let key_encoded1 = key.encode()?;
        let ns: &str = "ns2";
        let key = DbKey(addr, ns.as_bytes());
        let key_encoded2 = key.encode()?;
        assert!(key_encoded1.cmp(&key_encoded1) == std::cmp::Ordering::Equal);
        assert!(key_encoded1.cmp(&key_encoded2) == std::cmp::Ordering::Less);
        Ok(())
    }
}
