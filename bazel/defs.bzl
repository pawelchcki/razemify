"""Public Bazel-native Flutter and Dart APIs used by Razemify consumers."""

load(
    "@rules_dart//dart:defs.bzl",
    _dart_analysis_options = "dart_analysis_options",
    _dart_analyze_test = "dart_analyze_test",
    _dart_format_test = "dart_format_test",
)
load("@rules_flutter//flutter:android.bzl", _flutter_android_app = "flutter_android_app")
load(
    "@rules_flutter//flutter:defs.bzl",
    _flutter_application = "flutter_application",
    _flutter_library = "flutter_library",
    _flutter_plugin = "flutter_plugin",
    _flutter_test = "flutter_test",
)

dart_analysis_options = _dart_analysis_options
dart_analyze_test = _dart_analyze_test
dart_format_test = _dart_format_test
flutter_android_app = _flutter_android_app
flutter_application = _flutter_application
flutter_library = _flutter_library
flutter_plugin = _flutter_plugin
flutter_test = _flutter_test
