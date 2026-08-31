# Version-scoped copy of rules_flutter's generated Dart plugin target. The
# paired Android overlay adds a source-level dependency that auto-generation
# cannot infer from this Dart dependency graph.

load("@rules_flutter//flutter:defs.bzl", "flutter_plugin")

flutter_plugin(
    name = "{PKG}",
    srcs = glob(["lib/**/*.dart"], allow_empty = True),
    deps = [
        "@{HUB_NAME}__flutter//:flutter",
        "@{HUB_NAME}__flutter_plugin_android_lifecycle//:flutter_plugin_android_lifecycle",
        "@{HUB_NAME}__image_picker_platform_interface//:image_picker_platform_interface",
    ],
    package_name = "{PKG}",
    plugin_platforms_json = "{\"android\":{\"dartPluginClass\":\"ImagePickerAndroid\",\"package\":\"io.flutter.plugins.imagepicker\",\"pluginClass\":\"ImagePickerPlugin\"}}",
    language_version = "3.6",
    visibility = ["//visibility:public"],
)
