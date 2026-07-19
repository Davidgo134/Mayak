// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'session.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

T _$identity<T>(T value) => value;

final _privateConstructorUsedError = UnsupportedError(
    'It seems like you constructed your class using `MyClass._()`. This constructor is only meant to be used by freezed and you are not supposed to need it nor use it.\nPlease check the documentation here for more information: https://github.com/rrousselGit/freezed#adding-getters-and-methods-to-our-models');

/// @nodoc
mixin _$UploadEvent {
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(BigInt sent, BigInt total) progress,
    required TResult Function(int status, Uint8List body) done,
    required TResult Function(String message) error,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(BigInt sent, BigInt total)? progress,
    TResult? Function(int status, Uint8List body)? done,
    TResult? Function(String message)? error,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(BigInt sent, BigInt total)? progress,
    TResult Function(int status, Uint8List body)? done,
    TResult Function(String message)? error,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(UploadEvent_Progress value) progress,
    required TResult Function(UploadEvent_Done value) done,
    required TResult Function(UploadEvent_Error value) error,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(UploadEvent_Progress value)? progress,
    TResult? Function(UploadEvent_Done value)? done,
    TResult? Function(UploadEvent_Error value)? error,
  }) =>
      throw _privateConstructorUsedError;
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(UploadEvent_Progress value)? progress,
    TResult Function(UploadEvent_Done value)? done,
    TResult Function(UploadEvent_Error value)? error,
    required TResult orElse(),
  }) =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class $UploadEventCopyWith<$Res> {
  factory $UploadEventCopyWith(
          UploadEvent value, $Res Function(UploadEvent) then) =
      _$UploadEventCopyWithImpl<$Res, UploadEvent>;
}

/// @nodoc
class _$UploadEventCopyWithImpl<$Res, $Val extends UploadEvent>
    implements $UploadEventCopyWith<$Res> {
  _$UploadEventCopyWithImpl(this._value, this._then);

  // ignore: unused_field
  final $Val _value;
  // ignore: unused_field
  final $Res Function($Val) _then;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
}

/// @nodoc
abstract class _$$UploadEvent_ProgressImplCopyWith<$Res> {
  factory _$$UploadEvent_ProgressImplCopyWith(_$UploadEvent_ProgressImpl value,
          $Res Function(_$UploadEvent_ProgressImpl) then) =
      __$$UploadEvent_ProgressImplCopyWithImpl<$Res>;
  @useResult
  $Res call({BigInt sent, BigInt total});
}

/// @nodoc
class __$$UploadEvent_ProgressImplCopyWithImpl<$Res>
    extends _$UploadEventCopyWithImpl<$Res, _$UploadEvent_ProgressImpl>
    implements _$$UploadEvent_ProgressImplCopyWith<$Res> {
  __$$UploadEvent_ProgressImplCopyWithImpl(_$UploadEvent_ProgressImpl _value,
      $Res Function(_$UploadEvent_ProgressImpl) _then)
      : super(_value, _then);

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? sent = null,
    Object? total = null,
  }) {
    return _then(_$UploadEvent_ProgressImpl(
      sent: null == sent
          ? _value.sent
          : sent // ignore: cast_nullable_to_non_nullable
              as BigInt,
      total: null == total
          ? _value.total
          : total // ignore: cast_nullable_to_non_nullable
              as BigInt,
    ));
  }
}

/// @nodoc

class _$UploadEvent_ProgressImpl extends UploadEvent_Progress {
  const _$UploadEvent_ProgressImpl({required this.sent, required this.total})
      : super._();

  @override
  final BigInt sent;
  @override
  final BigInt total;

  @override
  String toString() {
    return 'UploadEvent.progress(sent: $sent, total: $total)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$UploadEvent_ProgressImpl &&
            (identical(other.sent, sent) || other.sent == sent) &&
            (identical(other.total, total) || other.total == total));
  }

  @override
  int get hashCode => Object.hash(runtimeType, sent, total);

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$UploadEvent_ProgressImplCopyWith<_$UploadEvent_ProgressImpl>
      get copyWith =>
          __$$UploadEvent_ProgressImplCopyWithImpl<_$UploadEvent_ProgressImpl>(
              this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(BigInt sent, BigInt total) progress,
    required TResult Function(int status, Uint8List body) done,
    required TResult Function(String message) error,
  }) {
    return progress(sent, total);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(BigInt sent, BigInt total)? progress,
    TResult? Function(int status, Uint8List body)? done,
    TResult? Function(String message)? error,
  }) {
    return progress?.call(sent, total);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(BigInt sent, BigInt total)? progress,
    TResult Function(int status, Uint8List body)? done,
    TResult Function(String message)? error,
    required TResult orElse(),
  }) {
    if (progress != null) {
      return progress(sent, total);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(UploadEvent_Progress value) progress,
    required TResult Function(UploadEvent_Done value) done,
    required TResult Function(UploadEvent_Error value) error,
  }) {
    return progress(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(UploadEvent_Progress value)? progress,
    TResult? Function(UploadEvent_Done value)? done,
    TResult? Function(UploadEvent_Error value)? error,
  }) {
    return progress?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(UploadEvent_Progress value)? progress,
    TResult Function(UploadEvent_Done value)? done,
    TResult Function(UploadEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (progress != null) {
      return progress(this);
    }
    return orElse();
  }
}

abstract class UploadEvent_Progress extends UploadEvent {
  const factory UploadEvent_Progress(
      {required final BigInt sent,
      required final BigInt total}) = _$UploadEvent_ProgressImpl;
  const UploadEvent_Progress._() : super._();

  BigInt get sent;
  BigInt get total;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$UploadEvent_ProgressImplCopyWith<_$UploadEvent_ProgressImpl>
      get copyWith => throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$UploadEvent_DoneImplCopyWith<$Res> {
  factory _$$UploadEvent_DoneImplCopyWith(_$UploadEvent_DoneImpl value,
          $Res Function(_$UploadEvent_DoneImpl) then) =
      __$$UploadEvent_DoneImplCopyWithImpl<$Res>;
  @useResult
  $Res call({int status, Uint8List body});
}

/// @nodoc
class __$$UploadEvent_DoneImplCopyWithImpl<$Res>
    extends _$UploadEventCopyWithImpl<$Res, _$UploadEvent_DoneImpl>
    implements _$$UploadEvent_DoneImplCopyWith<$Res> {
  __$$UploadEvent_DoneImplCopyWithImpl(_$UploadEvent_DoneImpl _value,
      $Res Function(_$UploadEvent_DoneImpl) _then)
      : super(_value, _then);

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? status = null,
    Object? body = null,
  }) {
    return _then(_$UploadEvent_DoneImpl(
      status: null == status
          ? _value.status
          : status // ignore: cast_nullable_to_non_nullable
              as int,
      body: null == body
          ? _value.body
          : body // ignore: cast_nullable_to_non_nullable
              as Uint8List,
    ));
  }
}

/// @nodoc

class _$UploadEvent_DoneImpl extends UploadEvent_Done {
  const _$UploadEvent_DoneImpl({required this.status, required this.body})
      : super._();

  @override
  final int status;
  @override
  final Uint8List body;

  @override
  String toString() {
    return 'UploadEvent.done(status: $status, body: $body)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$UploadEvent_DoneImpl &&
            (identical(other.status, status) || other.status == status) &&
            const DeepCollectionEquality().equals(other.body, body));
  }

  @override
  int get hashCode => Object.hash(
      runtimeType, status, const DeepCollectionEquality().hash(body));

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$UploadEvent_DoneImplCopyWith<_$UploadEvent_DoneImpl> get copyWith =>
      __$$UploadEvent_DoneImplCopyWithImpl<_$UploadEvent_DoneImpl>(
          this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(BigInt sent, BigInt total) progress,
    required TResult Function(int status, Uint8List body) done,
    required TResult Function(String message) error,
  }) {
    return done(status, body);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(BigInt sent, BigInt total)? progress,
    TResult? Function(int status, Uint8List body)? done,
    TResult? Function(String message)? error,
  }) {
    return done?.call(status, body);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(BigInt sent, BigInt total)? progress,
    TResult Function(int status, Uint8List body)? done,
    TResult Function(String message)? error,
    required TResult orElse(),
  }) {
    if (done != null) {
      return done(status, body);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(UploadEvent_Progress value) progress,
    required TResult Function(UploadEvent_Done value) done,
    required TResult Function(UploadEvent_Error value) error,
  }) {
    return done(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(UploadEvent_Progress value)? progress,
    TResult? Function(UploadEvent_Done value)? done,
    TResult? Function(UploadEvent_Error value)? error,
  }) {
    return done?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(UploadEvent_Progress value)? progress,
    TResult Function(UploadEvent_Done value)? done,
    TResult Function(UploadEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (done != null) {
      return done(this);
    }
    return orElse();
  }
}

abstract class UploadEvent_Done extends UploadEvent {
  const factory UploadEvent_Done(
      {required final int status,
      required final Uint8List body}) = _$UploadEvent_DoneImpl;
  const UploadEvent_Done._() : super._();

  int get status;
  Uint8List get body;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$UploadEvent_DoneImplCopyWith<_$UploadEvent_DoneImpl> get copyWith =>
      throw _privateConstructorUsedError;
}

/// @nodoc
abstract class _$$UploadEvent_ErrorImplCopyWith<$Res> {
  factory _$$UploadEvent_ErrorImplCopyWith(_$UploadEvent_ErrorImpl value,
          $Res Function(_$UploadEvent_ErrorImpl) then) =
      __$$UploadEvent_ErrorImplCopyWithImpl<$Res>;
  @useResult
  $Res call({String message});
}

/// @nodoc
class __$$UploadEvent_ErrorImplCopyWithImpl<$Res>
    extends _$UploadEventCopyWithImpl<$Res, _$UploadEvent_ErrorImpl>
    implements _$$UploadEvent_ErrorImplCopyWith<$Res> {
  __$$UploadEvent_ErrorImplCopyWithImpl(_$UploadEvent_ErrorImpl _value,
      $Res Function(_$UploadEvent_ErrorImpl) _then)
      : super(_value, _then);

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @pragma('vm:prefer-inline')
  @override
  $Res call({
    Object? message = null,
  }) {
    return _then(_$UploadEvent_ErrorImpl(
      message: null == message
          ? _value.message
          : message // ignore: cast_nullable_to_non_nullable
              as String,
    ));
  }
}

/// @nodoc

class _$UploadEvent_ErrorImpl extends UploadEvent_Error {
  const _$UploadEvent_ErrorImpl({required this.message}) : super._();

  @override
  final String message;

  @override
  String toString() {
    return 'UploadEvent.error(message: $message)';
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        (other.runtimeType == runtimeType &&
            other is _$UploadEvent_ErrorImpl &&
            (identical(other.message, message) || other.message == message));
  }

  @override
  int get hashCode => Object.hash(runtimeType, message);

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  @override
  @pragma('vm:prefer-inline')
  _$$UploadEvent_ErrorImplCopyWith<_$UploadEvent_ErrorImpl> get copyWith =>
      __$$UploadEvent_ErrorImplCopyWithImpl<_$UploadEvent_ErrorImpl>(
          this, _$identity);

  @override
  @optionalTypeArgs
  TResult when<TResult extends Object?>({
    required TResult Function(BigInt sent, BigInt total) progress,
    required TResult Function(int status, Uint8List body) done,
    required TResult Function(String message) error,
  }) {
    return error(message);
  }

  @override
  @optionalTypeArgs
  TResult? whenOrNull<TResult extends Object?>({
    TResult? Function(BigInt sent, BigInt total)? progress,
    TResult? Function(int status, Uint8List body)? done,
    TResult? Function(String message)? error,
  }) {
    return error?.call(message);
  }

  @override
  @optionalTypeArgs
  TResult maybeWhen<TResult extends Object?>({
    TResult Function(BigInt sent, BigInt total)? progress,
    TResult Function(int status, Uint8List body)? done,
    TResult Function(String message)? error,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(message);
    }
    return orElse();
  }

  @override
  @optionalTypeArgs
  TResult map<TResult extends Object?>({
    required TResult Function(UploadEvent_Progress value) progress,
    required TResult Function(UploadEvent_Done value) done,
    required TResult Function(UploadEvent_Error value) error,
  }) {
    return error(this);
  }

  @override
  @optionalTypeArgs
  TResult? mapOrNull<TResult extends Object?>({
    TResult? Function(UploadEvent_Progress value)? progress,
    TResult? Function(UploadEvent_Done value)? done,
    TResult? Function(UploadEvent_Error value)? error,
  }) {
    return error?.call(this);
  }

  @override
  @optionalTypeArgs
  TResult maybeMap<TResult extends Object?>({
    TResult Function(UploadEvent_Progress value)? progress,
    TResult Function(UploadEvent_Done value)? done,
    TResult Function(UploadEvent_Error value)? error,
    required TResult orElse(),
  }) {
    if (error != null) {
      return error(this);
    }
    return orElse();
  }
}

abstract class UploadEvent_Error extends UploadEvent {
  const factory UploadEvent_Error({required final String message}) =
      _$UploadEvent_ErrorImpl;
  const UploadEvent_Error._() : super._();

  String get message;

  /// Create a copy of UploadEvent
  /// with the given fields replaced by the non-null parameter values.
  @JsonKey(includeFromJson: false, includeToJson: false)
  _$$UploadEvent_ErrorImplCopyWith<_$UploadEvent_ErrorImpl> get copyWith =>
      throw _privateConstructorUsedError;
}
