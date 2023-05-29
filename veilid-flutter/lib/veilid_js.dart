import 'veilid.dart';

import 'dart:html' as html;
import 'dart:js' as js;
import 'dart:js_util' as js_util;
import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'veilid_encoding.dart';

//////////////////////////////////////////////////////////

Veilid getVeilid() => VeilidJS();

Object wasm = js_util.getProperty(html.window, "veilid_wasm");

Future<T> _wrapApiPromise<T>(Object p) {
  return js_util.promiseToFuture(p).then((value) => value as T).catchError(
      (error) => Future<T>.error(
          VeilidAPIException.fromJson(jsonDecode(error as String))));
}

class _Ctx {
  final int id;
  final VeilidJS js;
  _Ctx(this.id, this.js);
}

// JS implementation of VeilidRoutingContext
class VeilidRoutingContextJS implements VeilidRoutingContext {
  final _Ctx _ctx;
  static final Finalizer<_Ctx> _finalizer = Finalizer(
      (ctx) => js_util.callMethod(wasm, "release_routing_context", [ctx.id]));

  VeilidRoutingContextJS._(this._ctx) {
    _finalizer.attach(this, _ctx, detach: this);
  }

  @override
  VeilidRoutingContextJS withPrivacy() {
    int newId =
        js_util.callMethod(wasm, "routing_context_with_privacy", [_ctx.id]);
    return VeilidRoutingContextJS._(_Ctx(newId, _ctx.js));
  }

  @override
  VeilidRoutingContextJS withCustomPrivacy(Stability stability) {
    final newId = js_util.callMethod(
        wasm,
        "routing_context_with_custom_privacy",
        [_ctx.id, jsonEncode(stability)]);

    return VeilidRoutingContextJS._(_Ctx(newId, _ctx.js));
  }

  @override
  VeilidRoutingContextJS withSequencing(Sequencing sequencing) {
    final newId = js_util.callMethod(wasm, "routing_context_with_sequencing",
        [_ctx.id, jsonEncode(sequencing)]);
    return VeilidRoutingContextJS._(_Ctx(newId, _ctx.js));
  }

  @override
  Future<Uint8List> appCall(String target, Uint8List request) async {
    var encodedRequest = base64UrlNoPadEncode(request);

    return base64UrlNoPadDecode(await _wrapApiPromise(js_util.callMethod(
        wasm, "routing_context_app_call", [_ctx.id, target, encodedRequest])));
  }

  @override
  Future<void> appMessage(String target, Uint8List message) {
    var encodedMessage = base64UrlNoPadEncode(message);

    return _wrapApiPromise(js_util.callMethod(wasm,
        "routing_context_app_message", [_ctx.id, target, encodedMessage]));
  }

  @override
  Future<DHTRecordDescriptor> createDHTRecord(
      CryptoKind kind, DHTSchema schema) async {
    return DHTRecordDescriptor.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "routing_context_create_dht_record",
            [_ctx.id, kind, jsonEncode(schema)]))));
  }

  @override
  Future<DHTRecordDescriptor> openDHTRecord(
      TypedKey key, KeyPair? writer) async {
    return DHTRecordDescriptor.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "routing_context_open_dht_record", [
      _ctx.id,
      jsonEncode(key),
      writer != null ? jsonEncode(writer) : null
    ]))));
  }

  @override
  Future<void> closeDHTRecord(TypedKey key) {
    return _wrapApiPromise(js_util.callMethod(
        wasm, "routing_context_close_dht_record", [_ctx.id, jsonEncode(key)]));
  }

  @override
  Future<void> deleteDHTRecord(TypedKey key) {
    return _wrapApiPromise(js_util.callMethod(
        wasm, "routing_context_delete_dht_record", [_ctx.id, jsonEncode(key)]));
  }

  @override
  Future<ValueData?> getDHTValue(
      TypedKey key, int subkey, bool forceRefresh) async {
    final opt = await _wrapApiPromise(js_util.callMethod(
        wasm,
        "routing_context_get_dht_value",
        [_ctx.id, jsonEncode(key), subkey, forceRefresh]));
    return opt == null ? null : ValueData.fromJson(jsonDecode(opt));
  }

  @override
  Future<ValueData?> setDHTValue(
      TypedKey key, int subkey, Uint8List data) async {
    final opt = await _wrapApiPromise(js_util.callMethod(
        wasm,
        "routing_context_set_dht_value",
        [_ctx.id, jsonEncode(key), subkey, base64UrlNoPadEncode(data)]));
    return opt == null ? null : ValueData.fromJson(jsonDecode(opt));
  }

  @override
  Future<Timestamp> watchDHTValues(TypedKey key, ValueSubkeyRange subkeys,
      Timestamp expiration, int count) async {
    final ts = await _wrapApiPromise(js_util.callMethod(
        wasm, "routing_context_watch_dht_values", [
      _ctx.id,
      jsonEncode(key),
      jsonEncode(subkeys),
      expiration.toString(),
      count
    ]));
    return Timestamp.fromString(ts);
  }

  @override
  Future<bool> cancelDHTWatch(TypedKey key, ValueSubkeyRange subkeys) {
    return _wrapApiPromise(js_util.callMethod(
        wasm,
        "routing_context_cancel_dht_watch",
        [_ctx.id, jsonEncode(key), jsonEncode(subkeys)]));
  }
}

// JS implementation of VeilidCryptoSystem
class VeilidCryptoSystemJS implements VeilidCryptoSystem {
  final CryptoKind _kind;
  final VeilidJS _js;

  VeilidCryptoSystemJS._(this._js, this._kind) {
    // Keep the reference
    _js;
  }

  @override
  CryptoKind kind() {
    return _kind;
  }

  @override
  Future<SharedSecret> cachedDH(PublicKey key, SecretKey secret) async {
    return SharedSecret.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "crypto_cached_dh",
            [_kind, jsonEncode(key), jsonEncode(secret)]))));
  }

  @override
  Future<SharedSecret> computeDH(PublicKey key, SecretKey secret) async {
    return SharedSecret.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "crypto_compute_dh",
            [_kind, jsonEncode(key), jsonEncode(secret)]))));
  }

  @override
  Future<Uint8List> randomBytes(int len) async {
    return base64UrlNoPadDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "crypto_random_bytes", [_kind, len])));
  }

  @override
  Future<int> defaultSaltLength() {
    return _wrapApiPromise(
        js_util.callMethod(wasm, "crypto_default_salt_length", [_kind]));
  }

  @override
  Future<String> hashPassword(Uint8List password, Uint8List salt) {
    return _wrapApiPromise(js_util.callMethod(wasm, "crypto_hash_password",
        [_kind, base64UrlNoPadEncode(password), base64UrlNoPadEncode(salt)]));
  }

  @override
  Future<bool> verifyPassword(Uint8List password, String passwordHash) {
    return _wrapApiPromise(js_util.callMethod(wasm, "crypto_verify_password",
        [_kind, base64UrlNoPadEncode(password), passwordHash]));
  }

  @override
  Future<SharedSecret> deriveSharedSecret(
      Uint8List password, Uint8List salt) async {
    return SharedSecret.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "crypto_derive_shared_secret", [
      _kind,
      base64UrlNoPadEncode(password),
      base64UrlNoPadEncode(salt)
    ]))));
  }

  @override
  Future<Nonce> randomNonce() async {
    return Nonce.fromJson(jsonDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "crypto_random_nonce", [_kind]))));
  }

  @override
  Future<SharedSecret> randomSharedSecret() async {
    return SharedSecret.fromJson(jsonDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "crypto_random_shared_secret", [_kind]))));
  }

  @override
  Future<KeyPair> generateKeyPair() async {
    return KeyPair.fromJson(jsonDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "crypto_generate_key_pair", [_kind]))));
  }

  @override
  Future<HashDigest> generateHash(Uint8List data) async {
    return HashDigest.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "crypto_generate_hash",
            [_kind, base64UrlNoPadEncode(data)]))));
  }

  @override
  Future<bool> validateKeyPair(PublicKey key, SecretKey secret) {
    return _wrapApiPromise(js_util.callMethod(wasm, "crypto_validate_key_pair",
        [_kind, jsonEncode(key), jsonEncode(secret)]));
  }

  @override
  Future<bool> validateHash(Uint8List data, HashDigest hash) {
    return _wrapApiPromise(js_util.callMethod(wasm, "crypto_validate_hash",
        [_kind, base64UrlNoPadEncode(data), jsonEncode(hash)]));
  }

  @override
  Future<CryptoKeyDistance> distance(CryptoKey key1, CryptoKey key2) async {
    return CryptoKeyDistance.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "crypto_distance",
            [_kind, jsonEncode(key1), jsonEncode(key2)]))));
  }

  @override
  Future<Signature> sign(
      PublicKey key, SecretKey secret, Uint8List data) async {
    return Signature.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(wasm, "crypto_sign", [
      _kind,
      jsonEncode(key),
      jsonEncode(secret),
      base64UrlNoPadEncode(data)
    ]))));
  }

  @override
  Future<void> verify(PublicKey key, Uint8List data, Signature signature) {
    return _wrapApiPromise(js_util.callMethod(wasm, "crypto_verify", [
      _kind,
      jsonEncode(key),
      base64UrlNoPadEncode(data),
      jsonEncode(signature),
    ]));
  }

  @override
  Future<int> aeadOverhead() {
    return _wrapApiPromise(
        js_util.callMethod(wasm, "crypto_aead_overhead", [_kind]));
  }

  @override
  Future<Uint8List> decryptAead(Uint8List body, Nonce nonce,
      SharedSecret sharedSecret, Uint8List? associatedData) async {
    return base64UrlNoPadDecode(
        await _wrapApiPromise(js_util.callMethod(wasm, "crypto_decrypt_aead", [
      _kind,
      base64UrlNoPadEncode(body),
      jsonEncode(nonce),
      jsonEncode(sharedSecret),
      associatedData != null ? base64UrlNoPadEncode(associatedData) : null
    ])));
  }

  @override
  Future<Uint8List> encryptAead(Uint8List body, Nonce nonce,
      SharedSecret sharedSecret, Uint8List? associatedData) async {
    return base64UrlNoPadDecode(
        await _wrapApiPromise(js_util.callMethod(wasm, "crypto_encrypt_aead", [
      _kind,
      base64UrlNoPadEncode(body),
      jsonEncode(nonce),
      jsonEncode(sharedSecret),
      associatedData != null ? base64UrlNoPadEncode(associatedData) : null
    ])));
  }

  @override
  Future<Uint8List> cryptNoAuth(
      Uint8List body, Nonce nonce, SharedSecret sharedSecret) async {
    return base64UrlNoPadDecode(await _wrapApiPromise(js_util.callMethod(
        wasm, "crypto_crypt_no_auth", [
      _kind,
      base64UrlNoPadEncode(body),
      jsonEncode(nonce),
      jsonEncode(sharedSecret)
    ])));
  }
}

class _TDBT {
  final int id;
  VeilidTableDBJS tdbjs;
  VeilidJS js;

  _TDBT(this.id, this.tdbjs, this.js);
}

// JS implementation of VeilidTableDBTransaction
class VeilidTableDBTransactionJS extends VeilidTableDBTransaction {
  final _TDBT _tdbt;
  static final Finalizer<_TDBT> _finalizer = Finalizer((tdbt) =>
      js_util.callMethod(wasm, "release_table_db_transaction", [tdbt.id]));

  VeilidTableDBTransactionJS._(this._tdbt) {
    _finalizer.attach(this, _tdbt, detach: this);
  }

  @override
  Future<void> commit() {
    return _wrapApiPromise(
        js_util.callMethod(wasm, "table_db_transaction_commit", [_tdbt.id]));
  }

  @override
  Future<void> rollback() {
    return _wrapApiPromise(
        js_util.callMethod(wasm, "table_db_transaction_rollback", [_tdbt.id]));
  }

  @override
  Future<void> store(int col, Uint8List key, Uint8List value) {
    final encodedKey = base64UrlNoPadEncode(key);
    final encodedValue = base64UrlNoPadEncode(value);

    return _wrapApiPromise(js_util.callMethod(
        wasm,
        "table_db_transaction_store",
        [_tdbt.id, col, encodedKey, encodedValue]));
  }

  @override
  Future<bool> delete(int col, Uint8List key) {
    final encodedKey = base64UrlNoPadEncode(key);

    return _wrapApiPromise(js_util.callMethod(
        wasm, "table_db_transaction_delete", [_tdbt.id, col, encodedKey]));
  }
}

class _TDB {
  final int id;
  VeilidJS js;

  _TDB(this.id, this.js);
}

// JS implementation of VeilidTableDB
class VeilidTableDBJS extends VeilidTableDB {
  final _TDB _tdb;
  static final Finalizer<_TDB> _finalizer = Finalizer(
      (tdb) => js_util.callMethod(wasm, "release_table_db", [tdb.id]));

  VeilidTableDBJS._(this._tdb) {
    _finalizer.attach(this, _tdb, detach: this);
  }

  @override
  int getColumnCount() {
    return js_util.callMethod(wasm, "table_db_get_column_count", [_tdb.id]);
  }

  @override
  Future<List<Uint8List>> getKeys(int col) async {
    return jsonListConstructor(base64UrlNoPadDecodeDynamic)(jsonDecode(
        await js_util.callMethod(wasm, "table_db_get_keys", [_tdb.id, col])));
  }

  @override
  VeilidTableDBTransaction transact() {
    final id = js_util.callMethod(wasm, "table_db_transact", [_tdb.id]);

    return VeilidTableDBTransactionJS._(_TDBT(id, this, _tdb.js));
  }

  @override
  Future<void> store(int col, Uint8List key, Uint8List value) {
    final encodedKey = base64UrlNoPadEncode(key);
    final encodedValue = base64UrlNoPadEncode(value);

    return _wrapApiPromise(js_util.callMethod(
        wasm, "table_db_store", [_tdb.id, col, encodedKey, encodedValue]));
  }

  @override
  Future<Uint8List?> load(int col, Uint8List key) async {
    final encodedKey = base64UrlNoPadEncode(key);

    String? out = await _wrapApiPromise(
        js_util.callMethod(wasm, "table_db_load", [_tdb.id, col, encodedKey]));
    if (out == null) {
      return null;
    }
    return base64UrlNoPadDecode(out);
  }

  @override
  Future<Uint8List?> delete(int col, Uint8List key) {
    final encodedKey = base64UrlNoPadEncode(key);

    return _wrapApiPromise(js_util
        .callMethod(wasm, "table_db_delete", [_tdb.id, col, encodedKey]));
  }
}

// JS implementation of high level Veilid API

class VeilidJS implements Veilid {
  @override
  void initializeVeilidCore(Map<String, dynamic> platformConfigJson) {
    var platformConfigJsonString = jsonEncode(platformConfigJson);
    js_util
        .callMethod(wasm, "initialize_veilid_core", [platformConfigJsonString]);
  }

  @override
  void changeLogLevel(String layer, VeilidConfigLogLevel logLevel) {
    var logLevelJsonString = jsonEncode(logLevel);
    js_util.callMethod(wasm, "change_log_level", [layer, logLevelJsonString]);
  }

  @override
  Future<Stream<VeilidUpdate>> startupVeilidCore(VeilidConfig config) async {
    var streamController = StreamController<VeilidUpdate>();
    updateCallback(String update) {
      var updateJson = jsonDecode(update);
      if (updateJson["kind"] == "Shutdown") {
        streamController.close();
      } else {
        var update = VeilidUpdate.fromJson(updateJson);
        streamController.add(update);
      }
    }

    await _wrapApiPromise(js_util.callMethod(wasm, "startup_veilid_core",
        [js.allowInterop(updateCallback), jsonEncode(config)]));

    return streamController.stream;
  }

  @override
  Future<VeilidState> getVeilidState() async {
    return VeilidState.fromJson(jsonDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "get_veilid_state", []))));
  }

  @override
  Future<void> attach() {
    return _wrapApiPromise(js_util.callMethod(wasm, "attach", []));
  }

  @override
  Future<void> detach() {
    return _wrapApiPromise(js_util.callMethod(wasm, "detach", []));
  }

  @override
  Future<void> shutdownVeilidCore() {
    return _wrapApiPromise(
        js_util.callMethod(wasm, "shutdown_veilid_core", []));
  }

  @override
  List<CryptoKind> validCryptoKinds() {
    return jsonDecode(js_util.callMethod(wasm, "valid_crypto_kinds", []));
  }

  @override
  Future<VeilidCryptoSystem> getCryptoSystem(CryptoKind kind) async {
    if (!validCryptoKinds().contains(kind)) {
      throw VeilidAPIExceptionGeneric("unsupported cryptosystem");
    }
    return VeilidCryptoSystemJS._(this, kind);
  }

  @override
  Future<VeilidCryptoSystem> bestCryptoSystem() async {
    return VeilidCryptoSystemJS._(
        this, js_util.callMethod(wasm, "best_crypto_kind", []));
  }

  @override
  Future<List<TypedKey>> verifySignatures(List<TypedKey> nodeIds,
      Uint8List data, List<TypedSignature> signatures) async {
    return jsonListConstructor(TypedKey.fromJson)(jsonDecode(
        await _wrapApiPromise(js_util.callMethod(wasm, "verify_signatures", [
      jsonEncode(nodeIds),
      base64UrlNoPadEncode(data),
      jsonEncode(signatures)
    ]))));
  }

  @override
  Future<List<TypedSignature>> generateSignatures(
      Uint8List data, List<TypedKeyPair> keyPairs) async {
    return jsonListConstructor(TypedSignature.fromJson)(jsonDecode(
        await _wrapApiPromise(js_util.callMethod(wasm, "generate_signatures",
            [base64UrlNoPadEncode(data), jsonEncode(keyPairs)]))));
  }

  @override
  Future<TypedKeyPair> generateKeyPair(CryptoKind kind) async {
    return TypedKeyPair.fromJson(jsonDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "generate_key_pair", [kind]))));
  }

  @override
  Future<VeilidRoutingContext> routingContext() async {
    int id =
        await _wrapApiPromise(js_util.callMethod(wasm, "routing_context", []));
    return VeilidRoutingContextJS._(_Ctx(id, this));
  }

  @override
  Future<RouteBlob> newPrivateRoute() async {
    return RouteBlob.fromJson(jsonDecode(await _wrapApiPromise(
        js_util.callMethod(wasm, "new_private_route", []))));
  }

  @override
  Future<RouteBlob> newCustomPrivateRoute(
      Stability stability, Sequencing sequencing) async {
    var stabilityString = jsonEncode(stability);
    var sequencingString = jsonEncode(sequencing);

    return RouteBlob.fromJson(jsonDecode(await _wrapApiPromise(js_util
        .callMethod(
            wasm, "new_private_route", [stabilityString, sequencingString]))));
  }

  @override
  Future<String> importRemotePrivateRoute(Uint8List blob) {
    var encodedBlob = base64UrlNoPadEncode(blob);
    return _wrapApiPromise(
        js_util.callMethod(wasm, "import_remote_private_route", [encodedBlob]));
  }

  @override
  Future<void> releasePrivateRoute(String key) {
    return _wrapApiPromise(
        js_util.callMethod(wasm, "release_private_route", [key]));
  }

  @override
  Future<void> appCallReply(String id, Uint8List message) {
    var encodedMessage = base64UrlNoPadEncode(message);
    return _wrapApiPromise(
        js_util.callMethod(wasm, "app_call_reply", [id, encodedMessage]));
  }

  @override
  Future<VeilidTableDB> openTableDB(String name, int columnCount) async {
    int id = await _wrapApiPromise(
        js_util.callMethod(wasm, "open_table_db", [name, columnCount]));
    return VeilidTableDBJS._(_TDB(id, this));
  }

  @override
  Future<bool> deleteTableDB(String name) {
    return _wrapApiPromise(js_util.callMethod(wasm, "delete_table_db", [name]));
  }

  @override
  Timestamp now() {
    return Timestamp.fromString(js_util.callMethod(wasm, "now", []));
  }

  @override
  Future<String> debug(String command) async {
    return await _wrapApiPromise(js_util.callMethod(wasm, "debug", [command]));
  }

  @override
  String veilidVersionString() {
    return js_util.callMethod(wasm, "veilid_version_string", []);
  }

  @override
  VeilidVersion veilidVersion() {
    Map<String, dynamic> jsonVersion =
        jsonDecode(js_util.callMethod(wasm, "veilid_version", []));
    return VeilidVersion(
        jsonVersion["major"], jsonVersion["minor"], jsonVersion["patch"]);
  }
}
