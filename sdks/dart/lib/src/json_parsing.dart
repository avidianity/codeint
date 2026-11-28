import 'errors.dart';

abstract class JsonModel {
  const JsonModel(this.json);

  final Map<String, Object?> json;

  Map<String, Object?> toJson() => json;

  String str(String key) => readString(json, key);
  String? optStr(String key) => readOptionalString(json, key);
  int intOf(String key) => readInt(json, key);
  int? optInt(String key) => readOptionalInt(json, key);
  double dbl(String key) => readDouble(json, key);
  double? optDbl(String key) => readOptionalDouble(json, key);
  bool flag(String key) => readBool(json, key);
  bool? optFlag(String key) => readOptionalBool(json, key);
  List<String> strs(String key) => readStringList(json, key);
  List<String> optStrs(String key) => readOptionalStringList(json, key);
  List<T> list<T>(String key, T Function(Map<String, Object?>) parse) =>
      readObjectList(json, key, parse);
  List<T> optList<T>(String key, T Function(Map<String, Object?>) parse) =>
      readOptionalObjectList(json, key, parse);
  T obj<T>(String key, T Function(Map<String, Object?>) parse) =>
      parse(requireJsonMap(json[key], key));
  T? optObj<T>(String key, T Function(Map<String, Object?>) parse) {
    final map = readOptionalMap(json, key);
    return map == null ? null : parse(map);
  }
}

Map<String, Object?> requireJsonMap(Object? value, String context) {
  if (value is Map<String, Object?>) {
    return value;
  }
  if (value is Map) {
    return value.map((key, entry) => MapEntry(key.toString(), entry));
  }
  throw CodeintError('Expected JSON object for $context');
}

Map<String, Object?>? readOptionalMap(
  Map<String, Object?> json,
  String key,
) {
  final value = json[key];
  return value == null ? null : requireJsonMap(value, key);
}

T readTyped<T>(Map<String, Object?> json, String key, String label) {
  final value = json[key];
  if (value is T) {
    return value;
  }
  throw CodeintError('Expected $label for "$key"');
}

T? readOptionalTyped<T extends Object>(
  Map<String, Object?> json,
  String key,
  String label,
) {
  final value = json[key];
  if (value == null) {
    return null;
  }
  if (value is T) {
    return value;
  }
  throw CodeintError('Expected optional $label for "$key"');
}

String readString(Map<String, Object?> json, String key) =>
    readTyped<String>(json, key, 'string');

String? readOptionalString(Map<String, Object?> json, String key) =>
    readOptionalTyped<String>(json, key, 'string');

bool readBool(Map<String, Object?> json, String key) =>
    readTyped<bool>(json, key, 'boolean');

bool? readOptionalBool(Map<String, Object?> json, String key) =>
    readOptionalTyped<bool>(json, key, 'boolean');

int readInt(Map<String, Object?> json, String key) {
  final value = readTyped<num>(json, key, 'integer');
  if (value is int) {
    return value;
  }
  if (value == value.toInt()) {
    return value.toInt();
  }
  throw CodeintError('Expected integer for "$key"');
}

int? readOptionalInt(Map<String, Object?> json, String key) =>
    json[key] == null ? null : readInt(json, key);

double readDouble(Map<String, Object?> json, String key) =>
    readTyped<num>(json, key, 'number').toDouble();

double? readOptionalDouble(Map<String, Object?> json, String key) =>
    json[key] == null ? null : readDouble(json, key);

List<String> readStringList(Map<String, Object?> json, String key) =>
    readTyped<List<Object?>>(json, key, 'string list')
        .map((item) => '$item')
        .toList(growable: false);

List<String> readOptionalStringList(Map<String, Object?> json, String key) =>
    json[key] == null ? const <String>[] : readStringList(json, key);

List<T> readObjectList<T>(
  Map<String, Object?> json,
  String key,
  T Function(Map<String, Object?>) parse,
) =>
    readTyped<List<Object?>>(json, key, 'object list')
        .map((item) => parse(requireJsonMap(item, '$key[]')))
        .toList(growable: false);

List<T> readOptionalObjectList<T>(
  Map<String, Object?> json,
  String key,
  T Function(Map<String, Object?>) parse,
) =>
    json[key] == null ? <T>[] : readObjectList(json, key, parse);

Map<String, T> readObjectMap<T>(
  Map<String, Object?> json,
  String key,
  T Function(Map<String, Object?>) parse,
) =>
    requireJsonMap(json[key], key).map(
      (entryKey, entryValue) => MapEntry(
        entryKey,
        parse(requireJsonMap(entryValue, '$key.$entryKey')),
      ),
    );

Map<String, T> readOptionalObjectMap<T>(
  Map<String, Object?> json,
  String key,
  T Function(Map<String, Object?>) parse,
) =>
    json[key] == null ? <String, T>{} : readObjectMap(json, key, parse);

Map<String, int> readOptionalIntMap(Map<String, Object?> json, String key) {
  final map = readOptionalMap(json, key);
  if (map == null) {
    return const <String, int>{};
  }
  return map.map((k, _) => MapEntry(k, readInt(map, k)));
}

Map<String, String> readOptionalStringMap(
  Map<String, Object?> json,
  String key,
) {
  final map = readOptionalMap(json, key);
  if (map == null) {
    return const <String, String>{};
  }
  return map.map((k, v) => MapEntry(k, '$v'));
}

Map<String, T> readUnknownEntries<T>(
  Map<String, Object?> json,
  Set<String> knownKeys,
  T Function(Map<String, Object?>) parse,
) {
  final result = <String, T>{};
  for (final entry in json.entries) {
    if (!knownKeys.contains(entry.key) && entry.value != null) {
      result[entry.key] = parse(requireJsonMap(entry.value, entry.key));
    }
  }
  return result;
}
