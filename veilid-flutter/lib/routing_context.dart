import 'dart:async';
import 'dart:typed_data';

import 'package:change_case/change_case.dart';
import 'package:equatable/equatable.dart';
import 'package:freezed_annotation/freezed_annotation.dart';

import 'veilid.dart';

part 'routing_context.freezed.dart';
part 'routing_context.g.dart';

//////////////////////////////////////

extension ValidateDFLT on DHTSchemaDFLT {
  bool validate() {
    if (oCnt > 65535) {
      return false;
    }
    if (oCnt <= 0) {
      return false;
    }
    return true;
  }
}

extension ValidateSMPL on DHTSchemaSMPL {
  bool validate() {
    final totalsv = members.fold(0, (acc, v) => acc + v.mCnt) + oCnt;
    if (totalsv > 65535) {
      return false;
    }
    if (totalsv <= 0) {
      return false;
    }
    return true;
  }
}

//////////////////////////////////////
/// DHT Schema

@Freezed(unionKey: 'kind', unionValueCase: FreezedUnionCase.pascal)
sealed class DHTSchema with _$DHTSchema {
  @FreezedUnionValue('DFLT')
  const factory DHTSchema.dflt({required int oCnt}) = DHTSchemaDFLT;

  @FreezedUnionValue('SMPL')
  const factory DHTSchema.smpl(
      {required int oCnt,
      required List<DHTSchemaMember> members}) = DHTSchemaSMPL;

  factory DHTSchema.fromJson(dynamic json) =>
      _$DHTSchemaFromJson(json as Map<String, dynamic>);
}

const DHTSchema defaultDHTSchema = DHTSchema.dflt(oCnt: 1);

@freezed
class DHTSchemaMember with _$DHTSchemaMember {
  @Assert('mCnt > 0 && mCnt <= 65535', 'value out of range')
  const factory DHTSchemaMember({
    required PublicKey mKey,
    required int mCnt,
  }) = _DHTSchemaMember;

  factory DHTSchemaMember.fromJson(dynamic json) =>
      _$DHTSchemaMemberFromJson(json as Map<String, dynamic>);
}

//////////////////////////////////////
/// DHTRecordDescriptor

@freezed
class DHTRecordDescriptor with _$DHTRecordDescriptor {
  const factory DHTRecordDescriptor({
    required TypedKey key,
    required PublicKey owner,
    required DHTSchema schema,
    PublicKey? ownerSecret,
  }) = _DHTRecordDescriptor;
  factory DHTRecordDescriptor.fromJson(dynamic json) =>
      _$DHTRecordDescriptorFromJson(json as Map<String, dynamic>);
}

extension DHTRecordDescriptorExt on DHTRecordDescriptor {
  KeyPair? ownerKeyPair() {
    if (ownerSecret == null) {
      return null;
    }
    return KeyPair(key: owner, secret: ownerSecret!);
  }

  TypedKeyPair? ownerTypedKeyPair() {
    if (ownerSecret == null) {
      return null;
    }
    return TypedKeyPair(kind: key.kind, key: owner, secret: ownerSecret!);
  }
}

//////////////////////////////////////
/// ValueSubkeyRange

@freezed
class ValueSubkeyRange with _$ValueSubkeyRange {
  @Assert('low < 0 || low > high', 'low out of range')
  @Assert('high < 0', 'high out of range')
  const factory ValueSubkeyRange({
    required int low,
    required int high,
  }) = _ValueSubkeyRange;

  factory ValueSubkeyRange.fromJson(dynamic json) =>
      _$ValueSubkeyRangeFromJson(json as Map<String, dynamic>);
}

//////////////////////////////////////
/// ValueData

@freezed
class ValueData with _$ValueData {
  @Assert('seq >= 0', 'seq out of range')
  const factory ValueData({
    required int seq,
    @Uint8ListJsonConverter.jsIsArray() required Uint8List data,
    required PublicKey writer,
  }) = _ValueData;

  factory ValueData.fromJson(dynamic json) =>
      _$ValueDataFromJson(json as Map<String, dynamic>);
}

//////////////////////////////////////
/// Stability

enum Stability {
  lowLatency,
  reliable;

  factory Stability.fromJson(dynamic j) =>
      Stability.values.byName((j as String).toCamelCase());
  String toJson() => name.toPascalCase();
}

//////////////////////////////////////
/// Sequencing

enum Sequencing {
  noPreference,
  preferOrdered,
  ensureOrdered;

  factory Sequencing.fromJson(dynamic j) =>
      Sequencing.values.byName((j as String).toCamelCase());
  String toJson() => name.toPascalCase();
}

//////////////////////////////////////
/// SafetySelection

@immutable
abstract class SafetySelection {
  factory SafetySelection.fromJson(dynamic jsond) {
    final json = jsond as Map<String, dynamic>;
    if (json.containsKey('Unsafe')) {
      return SafetySelectionUnsafe(
          sequencing: Sequencing.fromJson(json['Unsafe']));
    } else if (json.containsKey('Safe')) {
      return SafetySelectionSafe(safetySpec: SafetySpec.fromJson(json['Safe']));
    } else {
      throw const VeilidAPIExceptionInternal('Invalid SafetySelection');
    }
  }
  Map<String, dynamic> toJson();
}

@immutable
class SafetySelectionUnsafe extends Equatable implements SafetySelection {
  //
  const SafetySelectionUnsafe({
    required this.sequencing,
  });
  final Sequencing sequencing;
  @override
  List<Object> get props => [sequencing];
  @override
  bool? get stringify => null;

  @override
  Map<String, dynamic> toJson() => {'Unsafe': sequencing.toJson()};
}

@immutable
class SafetySelectionSafe extends Equatable implements SafetySelection {
  //
  const SafetySelectionSafe({
    required this.safetySpec,
  });
  final SafetySpec safetySpec;
  @override
  List<Object> get props => [safetySpec];
  @override
  bool? get stringify => null;

  @override
  Map<String, dynamic> toJson() => {'Safe': safetySpec.toJson()};
}

/// Options for safety routes (sender privacy)
@freezed
class SafetySpec with _$SafetySpec {
  const factory SafetySpec({
    required int hopCount,
    required Stability stability,
    required Sequencing sequencing,
    String? preferredRoute,
  }) = _SafetySpec;

  factory SafetySpec.fromJson(dynamic json) =>
      _$SafetySpecFromJson(json as Map<String, dynamic>);
}

//////////////////////////////////////
/// RouteBlob
@freezed
class RouteBlob with _$RouteBlob {
  const factory RouteBlob(
      {required String routeId,
      @Uint8ListJsonConverter() required Uint8List blob}) = _RouteBlob;
  factory RouteBlob.fromJson(dynamic json) =>
      _$RouteBlobFromJson(json as Map<String, dynamic>);
}

//////////////////////////////////////
/// VeilidRoutingContext

abstract class VeilidRoutingContext {
  void close();

  // Modifiers
  VeilidRoutingContext withDefaultSafety();
  VeilidRoutingContext withSafety(SafetySelection safetySelection);
  VeilidRoutingContext withSequencing(Sequencing sequencing);
  Future<SafetySelection> safety();

  // App call/message
  Future<Uint8List> appCall(String target, Uint8List request);
  Future<void> appMessage(String target, Uint8List message);

  // DHT Operations
  Future<DHTRecordDescriptor> createDHTRecord(DHTSchema schema,
      {CryptoKind kind = 0});
  Future<DHTRecordDescriptor> openDHTRecord(TypedKey key, KeyPair? writer);
  Future<void> closeDHTRecord(TypedKey key);
  Future<void> deleteDHTRecord(TypedKey key);
  Future<ValueData?> getDHTValue(TypedKey key, int subkey, bool forceRefresh);
  Future<ValueData?> setDHTValue(TypedKey key, int subkey, Uint8List data);
  Future<Timestamp> watchDHTValues(TypedKey key,
      {List<ValueSubkeyRange>? subkeys, Timestamp? expiration, int? count});
  Future<bool> cancelDHTWatch(TypedKey key, {List<ValueSubkeyRange>? subkeys});
}
