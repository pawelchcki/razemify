# Injected into the onnxruntime release archives (see MODULE.bazel).
filegroup(
    name = "onnxruntime",
    srcs = glob(["lib/libonnxruntime*"]),
    visibility = ["//visibility:public"],
)

# The single real (non-symlink) shared object, for $(rootpath) expansion.
# Each archive matches exactly one of these patterns; keep the version in sync
# with the pin in MODULE.bazel. allow_empty = False so a version bump that
# forgets this file fails here rather than silently yielding no dylib.
filegroup(
    name = "dylib",
    # One pattern, not one per layout: allow_empty applies per pattern, so a
    # macOS-only pattern would fail on the Linux archives and vice versa.
    # Matches libonnxruntime.so.<ver> and libonnxruntime.<ver>.dylib, and not
    # the unversioned symlinks beside them.
    srcs = glob(
        ["lib/libonnxruntime*1.28.0*"],
        allow_empty = False,
    ),
    visibility = ["//visibility:public"],
)
