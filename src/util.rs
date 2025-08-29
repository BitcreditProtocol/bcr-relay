use std::str::FromStr;

use nostr::hashes::{sha256, Hash};
use nostr::nips::nip19::FromBech32;
use nostr::secp256k1::{schnorr::Signature, Message, XOnlyPublicKey, SECP256K1};

const LOGO_FILE_NAME: &str = "static/logo.png";
const ANON_HEAD_TAIL: usize = 2;

pub fn validate_npub(npub: &str) -> Result<XOnlyPublicKey, anyhow::Error> {
    let parsed_npub = nostr::PublicKey::from_bech32(npub)?;
    let xonly = parsed_npub.xonly()?;
    Ok(xonly)
}

pub fn get_logo_link(host_url: &url::Url) -> String {
    host_url
        .join(LOGO_FILE_NAME)
        .expect("valid logo url")
        .to_string()
}

pub fn anonymize_npub(npub: &str) -> String {
    if let Some(last_n) = npub.char_indices().nth_back(ANON_HEAD_TAIL) {
        format!("npub1*******{}", &npub[last_n.0..])
    } else {
        "npub1*******".to_string()
    }
}

pub fn anonymize_email(email: &str) -> String {
    match email.split_once('@') {
        Some((before, after)) => {
            let first_n = match before.char_indices().nth(ANON_HEAD_TAIL) {
                Some(first_n) => &before[0..first_n.0],
                None => "",
            };

            let last_n = match after.char_indices().nth_back(ANON_HEAD_TAIL) {
                Some(last_n) => &after[last_n.0..],
                None => "",
            };
            format!("{}***@***{}", first_n, last_n)
        }
        None => "****@*****".to_string(),
    }
}

/// Verifies that the given challenge was signed using schnorr by the controller of pub_key's private key
pub fn verify_signature(
    challenge: &str,
    signature: &str,
    pub_key: &XOnlyPublicKey,
) -> Result<bool, anyhow::Error> {
    let msg = Message::from_digest_slice(&hex::decode(challenge)?)?;
    let decoded_signature = Signature::from_str(signature)?;
    Ok(SECP256K1
        .verify_schnorr(&decoded_signature, &msg, pub_key)
        .is_ok())
}

pub fn verify_request<Req>(
    req: &Req,
    signature: &str,
    key: &XOnlyPublicKey,
) -> Result<bool, anyhow::Error>
where
    Req: borsh::BorshSerialize,
{
    let serialized = borsh::to_vec(&req)?;
    let hash = sha256::Hash::hash(&serialized);
    let msg = Message::from_digest(*hash.as_ref());
    let decoded_signature = Signature::from_str(signature)?;

    Ok(SECP256K1
        .verify_schnorr(&decoded_signature, &msg, key)
        .is_ok())
}

#[cfg(test)]
pub mod tests {
    use std::str::FromStr;

    use crate::notification::NotificationSendPayload;

    use super::*;
    use nostr::{
        hashes::Hash,
        secp256k1::{Keypair, SecretKey},
    };
    use rand::RngCore;

    pub fn signature(challenge: &str, private_key: &SecretKey) -> String {
        let key_pair = Keypair::from_secret_key(SECP256K1, private_key);
        let msg = Message::from_digest_slice(&hex::decode(challenge).unwrap()).unwrap();
        SECP256K1.sign_schnorr(&msg, &key_pair).to_string()
    }

    pub fn sign_request<Req>(req: &Req, private_key: &SecretKey) -> String
    where
        Req: borsh::BorshSerialize,
    {
        let key_pair = Keypair::from_secret_key(SECP256K1, private_key);
        let serialized = borsh::to_vec(&req).unwrap();
        let hash: sha256::Hash = sha256::Hash::hash(&serialized);
        let req = Message::from_digest(*hash.as_ref());

        SECP256K1.sign_schnorr(&req, &key_pair).to_string()
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
        // print to be able to manually create requests with -- --nocapture
        println!("sig: {sig}");
        let verified = verify_signature(&challenge, &sig, &x_only_pub);
        assert!(verified.is_ok());
        assert!(verified.as_ref().unwrap());
    }

    #[test]
    fn sig_req_test() {
        let secret_key =
            SecretKey::from_str("8863c82829480536893fc49c4b30e244f97261e989433373d73c648c1a656a79")
                .unwrap();
        let x_only_pub = secret_key.public_key(SECP256K1).x_only_public_key().0;

        let req = NotificationSendPayload {
            kind: "BillAccepted".to_string(),
            id: "bitcrtB7nSVpa37KKGZvcz1Qz7TRRC3MvLp38FMJXbXiGaUQYt".to_string(),
            sender: "npub1ypdcmmqjhj0g086m29a2xgvj5f2saz9dem372nkzcu55sqjk3lhsu057p8".to_string(),
            receiver: "npub1ypdcmmqjhj0g086m29a2xgvj5f2saz9dem372nkzcu55sqjk3lhsu057p8".to_string(),
        };

        let sig = sign_request(&req, &secret_key);
        // print to be able to manually create requests with -- --nocapture
        println!("req sig: {sig}");
        let verified = verify_request(&req, &sig, &x_only_pub);
        assert!(verified.is_ok());
        assert!(verified.as_ref().unwrap());
    }

    #[test]
    fn anonymize_npub_test() {
        assert_eq!(
            anonymize_npub("npub1ypdcmmqjhj0g086m29a2xgvj5f2saz9dem372nkzcu55sqjk3lhsu057p8"),
            "npub1*******7p8"
        );
        assert_eq!(anonymize_npub("npub1ypdcmmqjhj0g0"), "npub1*******0g0");
        assert_eq!(anonymize_npub(""), "npub1*******");
    }

    #[test]
    fn anonymize_email_basic() {
        assert_eq!(anonymize_email("alice@example.com"), "al***@***com");
        assert_eq!(anonymize_email("ae@ee.at"), "***@***.at");
        assert_eq!(anonymize_email(""), "****@*****");
    }
}
