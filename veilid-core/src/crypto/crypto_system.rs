use super::*;

pub trait CryptoSystem {
    // Accessors
    fn kind(&self) -> CryptoKind;
    fn crypto(&self) -> Crypto;

    // Cached Operations
    fn cached_dh(
        &self,
        key: &PublicKey,
        secret: &SecretKey,
    ) -> Result<SharedSecret, VeilidAPIError>;

    // Generation
    fn random_bytes(&self, len: u32) -> Vec<u8>;
    fn default_salt_length(&self) -> u32;
    fn hash_password(&self, password: &[u8], salt: &[u8]) -> Result<String, VeilidAPIError>;
    fn verify_password(
        &self,
        password: &[u8],
        password_hash: String,
    ) -> Result<bool, VeilidAPIError>;
    fn derive_shared_secret(
        &self,
        password: &[u8],
        salt: &[u8],
    ) -> Result<SharedSecret, VeilidAPIError>;
    fn random_nonce(&self) -> Nonce;
    fn random_shared_secret(&self) -> SharedSecret;
    fn compute_dh(
        &self,
        key: &PublicKey,
        secret: &SecretKey,
    ) -> Result<SharedSecret, VeilidAPIError>;
    fn generate_keypair(&self) -> KeyPair;
    fn generate_hash(&self, data: &[u8]) -> HashDigest;
    fn generate_hash_reader(
        &self,
        reader: &mut dyn std::io::Read,
    ) -> Result<HashDigest, VeilidAPIError>;

    // Validation
    fn validate_keypair(&self, key: &PublicKey, secret: &SecretKey) -> bool;
    fn validate_hash(&self, data: &[u8], hash: &HashDigest) -> bool;
    fn validate_hash_reader(
        &self,
        reader: &mut dyn std::io::Read,
        hash: &HashDigest,
    ) -> Result<bool, VeilidAPIError>;

    // Distance Metric
    fn distance(&self, key1: &CryptoKey, key2: &CryptoKey) -> CryptoKeyDistance;

    // Authentication
    fn sign(
        &self,
        key: &PublicKey,
        secret: &SecretKey,
        data: &[u8],
    ) -> Result<Signature, VeilidAPIError>;
    fn verify(
        &self,
        key: &PublicKey,
        data: &[u8],
        signature: &Signature,
    ) -> Result<(), VeilidAPIError>;

    // AEAD Encrypt/Decrypt
    fn aead_overhead(&self) -> usize;
    fn decrypt_in_place_aead(
        &self,
        body: &mut Vec<u8>,
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<(), VeilidAPIError>;
    fn decrypt_aead(
        &self,
        body: &[u8],
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<Vec<u8>, VeilidAPIError>;
    fn encrypt_in_place_aead(
        &self,
        body: &mut Vec<u8>,
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<(), VeilidAPIError>;
    fn encrypt_aead(
        &self,
        body: &[u8],
        nonce: &Nonce,
        shared_secret: &SharedSecret,
        associated_data: Option<&[u8]>,
    ) -> Result<Vec<u8>, VeilidAPIError>;

    // NoAuth Encrypt/Decrypt
    fn crypt_in_place_no_auth(
        &self,
        body: &mut Vec<u8>,
        nonce: &Nonce,
        shared_secret: &SharedSecret,
    );
    fn crypt_b2b_no_auth(
        &self,
        in_buf: &[u8],
        out_buf: &mut [u8],
        nonce: &Nonce,
        shared_secret: &SharedSecret,
    );
    fn crypt_no_auth_aligned_8(
        &self,
        body: &[u8],
        nonce: &Nonce,
        shared_secret: &SharedSecret,
    ) -> Vec<u8>;
    fn crypt_no_auth_unaligned(
        &self,
        body: &[u8],
        nonce: &Nonce,
        shared_secret: &SharedSecret,
    ) -> Vec<u8>;
}
