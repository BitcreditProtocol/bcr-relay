use nostr::secp256k1::{schnorr::Signature, Message, XOnlyPublicKey, SECP256K1};

pub fn verify_signature(
    challenge: &str,
    signature: &str,
    pub_key: &XOnlyPublicKey,
) -> Result<bool, anyhow::Error> {
    let msg = Message::from_digest_slice(&hex::decode(challenge)?)?;
    let decoded_signature = Signature::from_slice(&hex::decode(signature)?)?;
    Ok(SECP256K1
        .verify_schnorr(&decoded_signature, &msg, pub_key)
        .is_ok())
}

#[cfg(test)]
pub mod tests {
    use std::str::FromStr;

    use super::*;
    use nostr::secp256k1::{Keypair, SecretKey};
    use rand::RngCore;

    pub fn signature(challenge: &str, private_key: &SecretKey) -> String {
        let key_pair = Keypair::from_secret_key(SECP256K1, private_key);
        let msg = Message::from_digest_slice(&hex::decode(challenge).unwrap()).unwrap();
        let signature = SECP256K1.sign_schnorr(&msg, &key_pair);
        hex::encode(signature.serialize())
    }

    #[test]
    fn sig_test() {
        let secret_key =
            SecretKey::from_str("8863c82829480536893fc49c4b30e244f97261e989433373d73c648c1a656a79")
                .unwrap();
        let x_only_pub = secret_key.public_key(SECP256K1).x_only_public_key().0;
        let mut random_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut random_bytes);

        let challenge = hex::encode(random_bytes);

        let sig = signature(&challenge, &secret_key);
        println!("sig: {sig}");
        let verified = verify_signature(&challenge, &sig, &x_only_pub);
        assert!(verified.is_ok());
        assert!(verified.as_ref().unwrap());
    }
}
