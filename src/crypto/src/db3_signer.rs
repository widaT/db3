//
// db3_signer.rs
// Copyright (C) 2023 db3.network Author imotai <codego.me@gmail.com>
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
//

use crate::db3_keypair::DB3KeyPair;
use crate::db3_signature::Signature;
use db3_error::{DB3Error, Result};
use signature::Signer;

pub struct Db3MultiSchemeSigner {
    kp: DB3KeyPair,
}

impl Db3MultiSchemeSigner {
    pub fn new(kp: DB3KeyPair) -> Self {
        Self { kp }
    }

    // sign msg
    pub fn sign(&self, msg: &[u8]) -> Result<Signature> {
        let signature: Signature = self
            .kp
            .try_sign(msg)
            .map_err(|e| DB3Error::SignMessageError(format!("{e}")))?;
        Ok(signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db3_signature::DB3Signature;
    use crate::key_derive;
    use crate::signature_scheme::SignatureScheme;
    use bytes::BytesMut;
    use db3_proto::db3_base_proto::{ChainId, ChainRole};
    use db3_proto::db3_mutation_proto::Mutation;
    use db3_proto::db3_mutation_proto::{KvPair, MutationAction};
    use prost::Message;
    fn db3_signer_smoke_test(scheme: &SignatureScheme) {
        let kv = KvPair {
            key: "k1".as_bytes().to_vec(),
            value: "value1".as_bytes().to_vec(),
            action: MutationAction::InsertKv.into(),
        };
        let mutation = Mutation {
            ns: "my_twitter".as_bytes().to_vec(),
            kv_pairs: vec![kv],
            nonce: 1,
            chain_id: ChainId::MainNet.into(),
            chain_role: ChainRole::StorageShardChain.into(),
            gas_price: None,
            gas: 10,
        };
        let mut buf = BytesMut::with_capacity(1024 * 8);
        mutation
            .encode(&mut buf)
            .map_err(|e| DB3Error::SignError(format!("{e}")))
            .unwrap();
        let buf = buf.freeze();
        let seed: [u8; 32] = [0; 32];
        let (address, keypair) =
            key_derive::derive_key_pair_from_path(&seed, None, scheme).unwrap();
        let signer = Db3MultiSchemeSigner::new(keypair);
        let signature_ret = signer.sign(&buf);
        assert_eq!(true, signature_ret.is_ok());
        let signature = signature_ret.unwrap();
        let result = signature.verify(&buf);
        assert_eq!(true, result.is_ok());
        assert_eq!(
            serde_json::to_string(&address).unwrap(),
            serde_json::to_string(&result.unwrap()).unwrap()
        );
    }

    #[test]
    fn db3_signer_ed25519_smoke_test() {
        db3_signer_smoke_test(&SignatureScheme::ED25519);
    }

    #[test]
    fn db3_signer_secp256k1_smoke_test() {
        db3_signer_smoke_test(&SignatureScheme::Secp256k1);
    }
}
