mod keys;
mod record;
mod record_data;
mod record_store;
mod record_store_limits;
mod tasks;
mod types;

use keys::*;
use record::*;
use record_data::*;
use record_store::*;
use record_store_limits::*;

pub use types::*;

use super::*;
use crate::rpc_processor::*;

/// The maximum size of a single subkey
const MAX_SUBKEY_SIZE: usize = ValueData::MAX_LEN;
/// The maximum total size of all subkeys of a record
const MAX_RECORD_DATA_SIZE: usize = 1_048_576;
/// Frequency to flush record stores to disk
const FLUSH_RECORD_STORES_INTERVAL_SECS: u32 = 1;

/// Locked structure for storage manager
struct StorageManagerInner {
    /// If we are started up
    initialized: bool,
    /// Records that have been 'created' or 'opened' by this node
    local_record_store: Option<RecordStore>,
    /// Records that have been pushed to this node for distribution by other nodes
    remote_record_store: Option<RecordStore>,
    /// RPC processor if it is available
    rpc_processor: Option<RPCProcessor>,
    /// Background processing task (not part of attachment manager tick tree so it happens when detached too)
    tick_future: Option<SendPinBoxFuture<()>>,
}

struct StorageManagerUnlockedInner {
    config: VeilidConfig,
    crypto: Crypto,
    protected_store: ProtectedStore,
    table_store: TableStore,
    block_store: BlockStore,

    // Background processes
    flush_record_stores_task: TickTask<EyreReport>,
}

#[derive(Clone)]
pub struct StorageManager {
    unlocked_inner: Arc<StorageManagerUnlockedInner>,
    inner: Arc<AsyncMutex<StorageManagerInner>>,
}

impl StorageManager {
    fn new_unlocked_inner(
        config: VeilidConfig,
        crypto: Crypto,
        protected_store: ProtectedStore,
        table_store: TableStore,
        block_store: BlockStore,
    ) -> StorageManagerUnlockedInner {
        StorageManagerUnlockedInner {
            config,
            crypto,
            protected_store,
            table_store,
            block_store,
            flush_record_stores_task: TickTask::new(FLUSH_RECORD_STORES_INTERVAL_SECS),
        }
    }
    fn new_inner() -> StorageManagerInner {
        StorageManagerInner {
            initialized: false,
            local_record_store: None,
            remote_record_store: None,
            rpc_processor: None,
            tick_future: None,
        }
    }

    fn local_limits_from_config(config: VeilidConfig) -> RecordStoreLimits {
        let c = config.get();
        RecordStoreLimits {
            subkey_cache_size: c.network.dht.local_subkey_cache_size as usize,
            max_subkey_size: MAX_SUBKEY_SIZE,
            max_record_total_size: MAX_RECORD_DATA_SIZE,
            max_records: None,
            max_subkey_cache_memory_mb: Some(
                c.network.dht.local_max_subkey_cache_memory_mb as usize,
            ),
            max_storage_space_mb: None,
        }
    }

    fn remote_limits_from_config(config: VeilidConfig) -> RecordStoreLimits {
        let c = config.get();
        RecordStoreLimits {
            subkey_cache_size: c.network.dht.remote_subkey_cache_size as usize,
            max_subkey_size: MAX_SUBKEY_SIZE,
            max_record_total_size: MAX_RECORD_DATA_SIZE,
            max_records: Some(c.network.dht.remote_max_records as usize),
            max_subkey_cache_memory_mb: Some(
                c.network.dht.remote_max_subkey_cache_memory_mb as usize,
            ),
            max_storage_space_mb: Some(c.network.dht.remote_max_storage_space_mb as usize),
        }
    }

    pub fn new(
        config: VeilidConfig,
        crypto: Crypto,
        protected_store: ProtectedStore,
        table_store: TableStore,
        block_store: BlockStore,
    ) -> StorageManager {
        let this = StorageManager {
            unlocked_inner: Arc::new(Self::new_unlocked_inner(
                config,
                crypto,
                protected_store,
                table_store,
                block_store,
            )),
            inner: Arc::new(AsyncMutex::new(Self::new_inner())),
        };

        this.setup_tasks();

        this
    }

    #[instrument(level = "debug", skip_all, err)]
    pub async fn init(&self) -> EyreResult<()> {
        debug!("startup storage manager");
        let mut inner = self.inner.lock().await;

        let local_limits = Self::local_limits_from_config(self.unlocked_inner.config.clone());
        let remote_limits = Self::remote_limits_from_config(self.unlocked_inner.config.clone());

        let mut local_record_store = RecordStore::new(
            self.unlocked_inner.table_store.clone(),
            "local",
            local_limits,
        );
        local_record_store.init().await?;

        let mut remote_record_store = RecordStore::new(
            self.unlocked_inner.table_store.clone(),
            "remote",
            remote_limits,
        );
        remote_record_store.init().await?;

        inner.local_record_store = Some(local_record_store);
        inner.remote_record_store = Some(remote_record_store);

        // Schedule tick
        let this = self.clone();
        let tick_future = interval(1000, move || {
            let this = this.clone();
            async move {
                if let Err(e) = this.tick().await {
                    warn!("storage manager tick failed: {}", e);
                }
            }
        });
        inner.tick_future = Some(tick_future);

        inner.initialized = true;

        Ok(())
    }

    pub async fn terminate(&self) {
        debug!("starting storage manager shutdown");

        let mut inner = self.inner.lock().await;

        // Stop ticker
        let tick_future = inner.tick_future.take();
        if let Some(f) = tick_future {
            f.await;
        }

        // Cancel all tasks
        self.cancel_tasks().await;

        // Release the storage manager
        *inner = Self::new_inner();

        debug!("finished storage manager shutdown");
    }

    pub async fn set_rpc_processor(&self, opt_rpc_processor: Option<RPCProcessor>) {
        let mut inner = self.inner.lock().await;
        inner.rpc_processor = opt_rpc_processor
    }

    /// # DHT Key = Hash(ownerKeyKind) of: [ ownerKeyValue, schema ]
    fn get_key(&self, vcrypto: CryptoSystemVersion, record: &Record) -> TypedKey {
        let compiled = record.descriptor().schema_data();
        let mut hash_data = Vec::<u8>::with_capacity(PUBLIC_KEY_LENGTH + 4 + compiled.len());
        hash_data.extend_from_slice(&vcrypto.kind().0);
        hash_data.extend_from_slice(&record.owner().bytes);
        hash_data.extend_from_slice(compiled);
        let hash = vcrypto.generate_hash(&hash_data);
        TypedKey::new(vcrypto.kind(), hash)
    }

    async fn new_local_record(
        &self,
        vcrypto: CryptoSystemVersion,
        record: Record,
    ) -> Result<TypedKey, VeilidAPIError> {
        // add value record to record store
        let mut inner = self.inner.lock().await;
        if !inner.initialized {
            apibail_generic!("not initialized");
        }
        let local_record_store = inner.local_record_store.as_mut().unwrap();
        let key = self.get_key(vcrypto.clone(), &record);
        local_record_store.new_record(key, record).await?;
        Ok(key)
    }

    pub async fn create_record(
        &self,
        kind: CryptoKind,
        schema: DHTSchema,
        safety_selection: SafetySelection,
    ) -> Result<TypedKey, VeilidAPIError> {
        // Get cryptosystem
        let Some(vcrypto) = self.unlocked_inner.crypto.get(kind) else {
            apibail_generic!("unsupported cryptosystem");
        };

        // Compile the dht schema
        let schema_data = schema.compile();

        // New values require a new owner key
        let owner = vcrypto.generate_keypair();

        // Make a signed value descriptor for this dht value
        let signed_value_descriptor = SignedValueDescriptor::make_signature(
            owner.key,
            schema_data,
            vcrypto.clone(),
            owner.secret,
        )?;

        // Add new local value record
        let cur_ts = get_aligned_timestamp();
        let record = Record::new(
            cur_ts,
            signed_value_descriptor,
            Some(owner.secret),
            safety_selection,
        )?;
        let dht_key = self
            .new_local_record(vcrypto, record)
            .await
            .map_err(VeilidAPIError::internal)?;

        Ok(dht_key)
    }

    pub async fn open_record(
        &self,
        key: TypedKey,
        secret: Option<SecretKey>,
        safety_selection: SafetySelection,
    ) -> Result<DHTRecordDescriptor, VeilidAPIError> {
        unimplemented!();
    }

    pub async fn close_record(&self, key: TypedKey) -> Result<(), VeilidAPIError> {
        unimplemented!();
    }

    pub async fn delete_record(&self, key: TypedKey) -> Result<(), VeilidAPIError> {
        unimplemented!();
    }

    pub async fn get_value(
        &self,
        key: TypedKey,
        subkey: ValueSubkey,
        force_refresh: bool,
    ) -> Result<Option<ValueData>, VeilidAPIError> {
        unimplemented!();
    }

    pub async fn set_value(
        &self,
        key: TypedKey,
        subkey: ValueSubkey,
        data: Vec<u8>,
    ) -> Result<Option<ValueData>, VeilidAPIError> {
        unimplemented!();
    }

    pub async fn watch_values(
        &self,
        key: TypedKey,
        subkeys: &[ValueSubkeyRange],
        expiration: Timestamp,
        count: u32,
    ) -> Result<Timestamp, VeilidAPIError> {
        unimplemented!();
    }

    pub async fn cancel_watch_values(
        &self,
        key: TypedKey,
        subkeys: &[ValueSubkeyRange],
    ) -> Result<bool, VeilidAPIError> {
        unimplemented!();
    }
}
