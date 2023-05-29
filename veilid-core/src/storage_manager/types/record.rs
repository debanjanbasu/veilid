use super::*;

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RkyvArchive, RkyvSerialize, RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct Record<D>
where
    D: Clone + RkyvArchive + RkyvSerialize<DefaultVeilidRkyvSerializer>,
    for<'t> <D as RkyvArchive>::Archived: CheckBytes<RkyvDefaultValidator<'t>>,
    <D as RkyvArchive>::Archived: RkyvDeserialize<D, VeilidSharedDeserializeMap>,
{
    descriptor: SignedValueDescriptor,
    subkey_count: usize,
    last_touched_ts: Timestamp,
    record_data_size: usize,
    detail: D,
}

impl<D> Record<D>
where
    D: Clone + RkyvArchive + RkyvSerialize<DefaultVeilidRkyvSerializer>,
    for<'t> <D as RkyvArchive>::Archived: CheckBytes<RkyvDefaultValidator<'t>>,
    <D as RkyvArchive>::Archived: RkyvDeserialize<D, VeilidSharedDeserializeMap>,
{
    pub fn new(
        cur_ts: Timestamp,
        descriptor: SignedValueDescriptor,
        detail: D,
    ) -> VeilidAPIResult<Self> {
        let schema = descriptor.schema()?;
        let subkey_count = schema.subkey_count();
        Ok(Self {
            descriptor,
            subkey_count,
            last_touched_ts: cur_ts,
            record_data_size: 0,
            detail,
        })
    }

    pub fn descriptor(&self) -> &SignedValueDescriptor {
        &self.descriptor
    }
    pub fn owner(&self) -> &PublicKey {
        self.descriptor.owner()
    }

    pub fn subkey_count(&self) -> usize {
        self.subkey_count
    }

    pub fn touch(&mut self, cur_ts: Timestamp) {
        self.last_touched_ts = cur_ts
    }

    pub fn last_touched(&self) -> Timestamp {
        self.last_touched_ts
    }

    pub fn set_record_data_size(&mut self, size: usize) {
        self.record_data_size = size;
    }

    pub fn record_data_size(&self) -> usize {
        self.record_data_size
    }

    pub fn schema(&self) -> DHTSchema {
        // unwrap is safe here because descriptor is immutable and set in new()
        self.descriptor.schema().unwrap()
    }

    pub fn total_size(&self) -> usize {
        mem::size_of::<Record<D>>() + self.descriptor.total_size() + self.record_data_size
    }

    pub fn detail(&self) -> &D {
        &self.detail
    }
    pub fn detail_mut(&mut self) -> &mut D {
        &mut self.detail
    }
}