"""Pinned, patched source repositories required by Razemify's Flutter API."""

load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")

_RULES_FLUTTER_COMMIT = "21c8082509bd3d377276c74153139fe6aafae756"
_HERMETIC_ANDROID_TOOLCHAINS_COMMIT = "c6a9f203fbe1d4e72ecfc8d6ea47b81c844435ff"

def _razemify_dependencies_impl(module_ctx):
    http_archive(
        name = "rules_flutter",
        patches = [Label("//third_party/rules_flutter:stage_path_dependencies.patch")],
        patch_args = ["-p1"],
        sha256 = "d0a87ec5cb086df23f987e4b42ad698420c64d8af627f1fb67e4ba59ded9b556",
        strip_prefix = "rules_flutter-{}".format(_RULES_FLUTTER_COMMIT),
        urls = ["https://github.com/pawelchcki/rules_flutter/archive/{}.tar.gz".format(_RULES_FLUTTER_COMMIT)],
    )

    http_archive(
        name = "hermetic_android_toolchains",
        integrity = "sha256-jovdpav966xUgaGTHIv5OMGtWxJMFk8ijbSoGQ6xX2E=",
        patches = [Label("//third_party/hermetic_android_toolchains:export_repositories.patch")],
        patch_args = ["-p1"],
        strip_prefix = "hermetic_android_toolchains-{}".format(_HERMETIC_ANDROID_TOOLCHAINS_COMMIT),
        urls = ["https://github.com/keith/hermetic_android_toolchains/archive/{}.tar.gz".format(_HERMETIC_ANDROID_TOOLCHAINS_COMMIT)],
    )

    return module_ctx.extension_metadata(reproducible = True)

razemify_dependencies = module_extension(
    implementation = _razemify_dependencies_impl,
)
