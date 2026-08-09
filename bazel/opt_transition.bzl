"""A narrow optimization transition for production native artifacts."""

def _opt_transition_impl(_settings, _attr):
    return {"//command_line_option:compilation_mode": "opt"}

_opt_transition = transition(
    implementation = _opt_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:compilation_mode"],
)

def _optimized_target_impl(ctx):
    target = ctx.attr.target[0][DefaultInfo]
    return [
        DefaultInfo(
            files = target.files,
            runfiles = target.default_runfiles,
        ),
    ]

optimized_target = rule(
    implementation = _optimized_target_impl,
    attrs = {
        "target": attr.label(
            cfg = _opt_transition,
            mandatory = True,
        ),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
)
