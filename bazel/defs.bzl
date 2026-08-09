"""Flutter rule API backed by the exact ruleset used to build Razemify."""

load(
    "@rules_flutter//flutter:defs.bzl",
    _DartLibraryInfo = "DartLibraryInfo",
    _DartProtoLibraryInfo = "DartProtoLibraryInfo",
    _FlutterLibraryInfo = "FlutterLibraryInfo",
    _dart_format_test = "dart_format_test",
    _dart_library = "dart_library",
    _dart_proto_library = "dart_proto_library",
    _flutter_analyze_test = "flutter_analyze_test",
    _flutter_app = "flutter_app",
    _flutter_build_settings = "flutter_build_settings",
    _flutter_goldens = "flutter_goldens",
    _flutter_library = "flutter_library",
    _flutter_test = "flutter_test",
)

DartLibraryInfo = _DartLibraryInfo
DartProtoLibraryInfo = _DartProtoLibraryInfo
FlutterLibraryInfo = _FlutterLibraryInfo
dart_format_test = _dart_format_test
dart_library = _dart_library
dart_proto_library = _dart_proto_library
flutter_analyze_test = _flutter_analyze_test
flutter_app = _flutter_app
flutter_build_settings = _flutter_build_settings
flutter_goldens = _flutter_goldens
flutter_library = _flutter_library
flutter_test = _flutter_test
