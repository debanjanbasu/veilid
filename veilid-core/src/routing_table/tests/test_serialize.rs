use crate::*;

fn fake_routing_table() -> routing_table::RoutingTable {
    let veilid_config = VeilidConfig::new();
    let block_store = BlockStore::new(veilid_config.clone());
    let protected_store = ProtectedStore::new(veilid_config.clone());
    let table_store = TableStore::new(veilid_config.clone());
    let crypto = Crypto::new(
        veilid_config.clone(),
        table_store.clone(),
        protected_store.clone(),
    );
    let storage_manager = storage_manager::StorageManager::new(
        veilid_config.clone(),
        crypto.clone(),
        protected_store.clone(),
        table_store.clone(),
        block_store.clone(),
    );
    let network_manager = network_manager::NetworkManager::new(
        veilid_config.clone(),
        storage_manager,
        protected_store.clone(),
        table_store.clone(),
        block_store.clone(),
        crypto.clone(),
    );
    routing_table::RoutingTable::new(network_manager)
}

pub async fn test_routingtable_buckets_round_trip() {
    let original = fake_routing_table();
    let copy = fake_routing_table();
    original.init().await.unwrap();
    copy.init().await.unwrap();

    // Add lots of routes to `original` here to exercise all various types.

    let (serialized_bucket_map, all_entry_bytes) = original.serialized_buckets().unwrap();

    copy.populate_routing_table(
        &mut copy.inner.write(),
        serialized_bucket_map,
        all_entry_bytes,
    )
    .unwrap();

    let original_inner = &*original.inner.read();
    let copy_inner = &*copy.inner.read();

    let routing_table_keys: Vec<_> = original_inner.buckets.keys().clone().collect();
    let copy_keys: Vec<_> = copy_inner.buckets.keys().clone().collect();

    assert_eq!(routing_table_keys.len(), copy_keys.len());

    for crypto in routing_table_keys {
        // The same keys are present in the original and copy RoutingTables.
        let original_buckets = original_inner.buckets.get(&crypto).unwrap();
        let copy_buckets = copy_inner.buckets.get(&crypto).unwrap();

        // Recurse into RoutingTable.inner.buckets
        for (left_buckets, right_buckets) in original_buckets.iter().zip(copy_buckets.iter()) {
            // Recurse into RoutingTable.inner.buckets.entries
            for ((left_crypto, left_entries), (right_crypto, right_entries)) in
                left_buckets.entries().zip(right_buckets.entries())
            {
                assert_eq!(left_crypto, right_crypto);

                assert_eq!(
                    format!("{:?}", left_entries),
                    format!("{:?}", right_entries)
                );
            }
        }
    }
}

pub async fn test_all() {
    test_routingtable_buckets_round_trip().await;
}
