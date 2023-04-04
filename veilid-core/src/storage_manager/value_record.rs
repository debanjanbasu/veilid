use super::*;

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    RkyvArchive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct ValueRecordData {
    data: ValueData,
    signature: Signature,
}

#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    RkyvArchive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct ValueRecord {
    last_touched_ts: Timestamp,
    secret: Option<SecretKey>,
    schema: DHTSchema,
    safety_selection: SafetySelection,
    data_size: usize,
}

impl ValueRecord {
    pub fn new(
        cur_ts: Timestamp,
        secret: Option<SecretKey>,
        schema: DHTSchema,
        safety_selection: SafetySelection,
    ) -> Self {
        Self {
            last_touched_ts: cur_ts,
            secret,
            schema,
            safety_selection,
            data_size: 0,
        }
    }

    pub fn subkey_count(&self) -> usize {
        self.schema.subkey_count()
    }

    pub fn touch(&mut self, cur_ts: Timestamp) {
        self.last_touched_ts = cur_ts
    }

    pub fn last_touched(&self) -> Timestamp {
        self.last_touched_ts
    }
}
