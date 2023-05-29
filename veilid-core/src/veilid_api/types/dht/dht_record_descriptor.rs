use super::*;

/// DHT Record Descriptor
#[derive(
    Debug,
    Clone,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    RkyvArchive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct DHTRecordDescriptor {
    /// DHT Key = Hash(ownerKeyKind) of: [ ownerKeyValue, schema ]
    key: TypedKey,
    /// The public key of the owner
    owner: PublicKey,
    /// If this key is being created: Some(the secret key of the owner)
    /// If this key is just being opened: None
    owner_secret: Option<SecretKey>,
    /// The schema in use associated with the key
    schema: DHTSchema,
}

impl DHTRecordDescriptor {
    pub fn new(
        key: TypedKey,
        owner: PublicKey,
        owner_secret: Option<SecretKey>,
        schema: DHTSchema,
    ) -> Self {
        Self {
            key,
            owner,
            owner_secret,
            schema,
        }
    }

    pub fn owner(&self) -> &PublicKey {
        &self.owner
    }

    pub fn owner_secret(&self) -> Option<&SecretKey> {
        self.owner_secret.as_ref()
    }

    pub fn schema(&self) -> &DHTSchema {
        &self.schema
    }
}