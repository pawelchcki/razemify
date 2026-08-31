# image_picker_android imports FlutterLifecycleAdapter from the lifecycle
# plugin's Android implementation. Bazel strict-deps requires that library as
# a direct edge; rules_flutter's generated target currently omits it.

load("@rules_flutter//flutter:android.bzl", "flutter_android_engine")
load("@rules_java//java:java_library.bzl", "java_library")
load("@rules_kotlin//kotlin:android.bzl", "kt_android_library")

flutter_android_engine(
    name = "_engine",
    visibility = ["//visibility:private"],
)

genrule(
    name = "_build_config_src",
    outs = ["_build_config/io/flutter/plugins/imagepicker/BuildConfig.java"],
    cmd = "cat > $@ <<'EOF'\npackage io.flutter.plugins.imagepicker;\npublic final class BuildConfig {\n  public static final boolean DEBUG = false;\n  public static final String LIBRARY_PACKAGE_NAME = \"io.flutter.plugins.imagepicker\";\n  public static final String BUILD_TYPE = \"release\";\n}\nEOF\n",
)

java_library(
    name = "_build_config",
    srcs = [":_build_config_src"],
    visibility = ["//visibility:private"],
)

kt_android_library(
    name = "lib",
    srcs = glob(["src/main/**/*.kt", "src/main/**/*.java"], allow_empty = True),
    custom_package = "io.flutter.plugins.imagepicker",
    manifest = "src/main/AndroidManifest.xml",
    exports_manifest = 1,
    resource_files = glob(["src/main/res/**"], allow_empty = False),
    visibility = ["//visibility:public"],
    deps = [
        ":_engine",
        ":_build_config",
        "@{HUB_NAME}__flutter_plugin_android_lifecycle//android:lib",
        "@rules_android_maven//:androidx_activity_activity",
        "@rules_android_maven//:androidx_annotation_annotation",
        "@rules_android_maven//:androidx_core_core",
    ],
)
