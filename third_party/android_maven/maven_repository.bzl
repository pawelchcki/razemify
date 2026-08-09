"""Restores a checksummed Maven directory from a URL manifest."""

def _maven_repository_impl(ctx):
    entries = json.decode(ctx.read(ctx.attr.manifest))
    downloads = []
    for entry in entries:
        downloads.append(ctx.download(
            block = False,
            url = entry["url"],
            integrity = entry["integrity"],
            output = entry["path"],
        ))
    for download in downloads:
        download.wait()
    ctx.file("BUILD.bazel", """package(default_visibility = [\"//visibility:public\"])
filegroup(name = \"mirror\", srcs = glob([\"**\"], exclude = [\"BUILD.bazel\"]))
""")

maven_repository = repository_rule(
    implementation = _maven_repository_impl,
    attrs = {"manifest": attr.label(mandatory = True, allow_single_file = True)},
)
