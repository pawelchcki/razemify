# Consuming Razemify with Bzlmod

Razemify is a Bazel module. Until it is published in a Bazel registry, pin the
repository itself from the consuming root module:

```starlark
bazel_dep(name = "razemify", version = "0.1.0")
git_override(
    module_name = "razemify",
    commit = "<reviewed-full-commit-sha>",
    remote = "https://github.com/pawelchcki/razemify.git",
)
```

The consuming repository must explicitly record acceptance of the Android SDK
and NDK licenses used by its toolchains:

```bazelrc
common --repo_env=ACCEPTED_ANDROID_SDK_LICENSE_VERSION=36
common --repo_env=ACCEPTED_ANDROID_NDK_LICENSE_VERSION=28.2.13676358
```

The Razemify module owns its patched `rules_flutter` and Android SDK-provider
sources. Consumers must use Razemify's re-exported extension and rule APIs so
both repositories share one set of Flutter providers and generated repos:

```starlark
flutter = use_extension("@razemify//bazel:flutter_extensions.bzl", "flutter")
flutter.android_toolchain(
    name = "android",
    build_tools_version = "36.0.0",
    gradle_distribution_integrity = "sha256-1yXXB7+r1N/clYxiQAOzyArMwD9wN7USLEsdDvFc7Ks=",
    gradle_distribution_url = "https://services.gradle.org/distributions/gradle-8.9-bin.zip",
    ndk_version = "28.2.13676358",
    sdk_version = "36",
)
use_repo(
    flutter,
    "android_android_ndk",
    "android_toolchains",
    "flutter_sdk",
    "flutter_toolchains",
)
register_toolchains(
    "@android_android_ndk//:all",
    "@android_toolchains//:all",
    "@flutter_toolchains//:all",
)

pub = use_extension("@razemify//bazel:flutter_extensions.bzl", "pub")
pub.lock(
    name = "razem_app_deps",
    file = "//:pubspec.lock",
)
use_repo(
    pub,
    "flutter_razemify_deps",
    "pub_fixnum",
    "pub_meta",
    "pub_path",
    "pub_protobuf",
    "pub_protoc_plugin",
    "razem_app_deps",
)
```

Load Flutter rules from Razemify's facade and add the matching plugin target to
the application library:

```starlark
load("@razemify//bazel:defs.bzl", "flutter_app", "flutter_library")

flutter_library(
    name = "lib_android_debug",
    srcs = glob(["lib/**"]),
    pubspec = "pubspec.yaml",
    deps = [
        "@flutter_sdk//flutter/packages/flutter",
        "@razem_app_deps//:all",
        "@razemify//:flutter_razemify_android_debug",
    ],
)

flutter_app(
    name = "app",
    apk = {
        "android_maven_repo": "@razem_app_android_maven//:mirror",
        "mode": "debug",
    },
    embed = [":lib_android_debug"],
)
```

Use `@razemify//:flutter_razemify_android_release` for release APKs or app
bundles, and `@razemify//:flutter_razemify` for non-Android Flutter targets.
The consuming root owns Android toolchain registration because Bzlmod and
`rules_flutter` intentionally reserve platform/toolchain policy for the root.
