# Consuming Razemify with Bzlmod

Razemify publishes a Bazel-native Flutter plugin at
`@razemify//:flutter_razemify`. It does not publish mode-specific or
ABI-specific plugin targets; Bazel's Android application transition selects
those properties for the whole dependency graph.

## Root module setup

Until Razemify is published in a Bazel registry, pin it from the consuming root
module. The root must also own the two fork overrides: Bzlmod intentionally
ignores overrides declared by dependencies.

```starlark
bazel_dep(name = "razemify", version = "0.1.0")
git_override(
    module_name = "razemify",
    commit = "<reviewed-full-commit-sha>",
    remote = "https://github.com/pawelchcki/razemify.git",
)

bazel_dep(name = "rules_flutter", version = "0.0.0")
archive_override(
    module_name = "rules_flutter",
    sha256 = "7e7c7bc1b12a636804227b820abceb8e3973af00ae41bfaa5e04edd6ab742b9b",
    strip_prefix = "rules_flutter-e81074d44674a2a295452931ddb6ab654ba089fc",
    urls = ["https://github.com/pawelchcki/rules_flutter/archive/e81074d44674a2a295452931ddb6ab654ba089fc.tar.gz"],
)

bazel_dep(name = "rules_dart", version = "0.4.9")
archive_override(
    module_name = "rules_dart",
    sha256 = "44b144bc5a4cce234160122a9f556a91c6b9a310bd262a3eacdc18625138b8ca",
    strip_prefix = "rules_dart-6cd2d1146614a6c982e492b2e558debf6f41d450",
    urls = ["https://github.com/aran/rules_dart/archive/6cd2d1146614a6c982e492b2e558debf6f41d450.tar.gz"],
)
```

Register Flutter 3.44.1 and resolve the consuming application's lock file with
the same extension:

```starlark
flutter = use_extension("@rules_flutter//flutter:extensions.bzl", "flutter")
flutter.toolchain(flutter_version = "3.44.1")
flutter.pub(name = "app_deps", lock = "//:pubspec.lock")
use_repo(
    flutter,
    "app_deps",
    "flutter_android_engine_arm64",
    "flutter_android_engine_x64",
    "flutter_toolchains",
)
register_toolchains("@flutter_toolchains//:all")
```

Android builds use `rules_android` 0.7.2, `rules_android_ndk` 0.1.5,
`rules_kotlin` 2.3.20, `rules_java` 9.6.1, `rules_jvm_external` 7.0, and
API 35. Configure the pinned hermetic toolchains in the root module:

```starlark
bazel_dep(name = "hermetic_android_toolchains", version = "0.0.0", dev_dependency = True)
bazel_dep(name = "rules_android", version = "0.7.2")
bazel_dep(name = "rules_android_ndk", version = "0.1.5")

archive_override(
    module_name = "hermetic_android_toolchains",
    sha256 = "7fce0f58bedeed8a5e2a00a2beae49c120784c4354c238b2604bf2aedcbfaa40",
    strip_prefix = "hermetic_android_toolchains-e723dd55401eaae04019e546e2f9bf78d0f33acf",
    urls = ["https://github.com/keith/hermetic_android_toolchains/archive/e723dd55401eaae04019e546e2f9bf78d0f33acf.tar.gz"],
)
single_version_override(module_name = "rules_android", version = "0.7.2")

android = use_extension(
    "@hermetic_android_toolchains//:extensions.bzl",
    "android",
    dev_dependency = True,
)
android.sdk(
    api_level = "35",
    build_tools_version = "35.0.0",
    version = "35",
)
android.ndk(version = "r28c")
use_repo(android, "androidsdk", "androidndk")

rules_android_sdk = use_extension(
    "@rules_android//rules/android_sdk_repository:rule.bzl",
    "android_sdk_repository_extension",
    dev_dependency = True,
)
override_repo(rules_android_sdk, "androidsdk")

rules_android_ndk = use_extension(
    "@rules_android_ndk//:extension.bzl",
    "android_ndk_repository_extension",
    dev_dependency = True,
)
override_repo(rules_android_ndk, "androidndk")

register_toolchains(
    "@androidndk//:all",
    "@androidsdk//:all",
    dev_dependency = True,
)
```

This downloads Android SDK 35 with build-tools 35.0.0 and NDK r28c
(28.2.13676358), then redirects the repositories expected by `rules_android`
and `rules_android_ndk` to those hermetic repositories.
`rules_android_ndk` 0.1.5 currently needs the archive override shown there.
The root must also carry the two `maven.install` declarations from that file:
they align rules_android's protobuf runtime with 4.34.0 and make plugin Android
artifacts available.

No host Android installation or `ANDROID_HOME`/`ANDROID_NDK_HOME` is used.
After reviewing and accepting Google's licenses for these versions, record the
accepted versions alongside the Java 25 and manifest settings in `.bazelrc`:

```bazelrc
common --repo_env=ACCEPTED_ANDROID_SDK_LICENSE_VERSION=35
common --repo_env=ACCEPTED_ANDROID_NDK_LICENSE_VERSION=r28c
common --java_runtime_version=remotejdk_25
common --tool_java_language_version=25
common --tool_java_runtime_version=remotejdk_25
common --merge_android_manifest_permissions
```

## Application targets

Load the public rules through Razemify's facade and make the public plugin a
dependency of the application. Use individual pub hub aliases rather than the
hub-wide `:all` target.

```starlark
load(
    "@razemify//bazel:defs.bzl",
    "flutter_android_app",
    "flutter_application",
)

flutter_application(
    name = "app_flutter",
    main = "lib/main.dart",
    package_name = "my_app",
    deps = [
        "@app_deps//:flutter",
        "@razemify//:flutter_razemify",
    ],
)

flutter_android_app(
    name = "app_arm64",
    android_abi = "arm64",
    application = ":app_flutter",
    package_name = "com.example.myapp",
    pub_hub_name = "app_deps",
)

flutter_android_app(
    name = "app_x64",
    android_abi = "x64",
    application = ":app_flutter",
    package_name = "com.example.myapp",
    pub_hub_name = "app_deps",
)
```

Select mode with Bazel's standard compilation flag and select ABI with the
target label:

```sh
bazel build -c dbg //:app_arm64
bazel build -c dbg //:app_x64
bazel build -c opt //:app_arm64
bazel build -c opt //:app_x64
```

The facade also exports `flutter_library`, `flutter_plugin`, `flutter_test`,
`dart_analysis_options`, `dart_analyze_test`, and `dart_format_test` for
consumers that want to keep their Flutter build and quality gates on the exact
rulesets Razemify uses.
