"""Razemify's shared rules_flutter module-extension identities."""

load(
    "@rules_flutter//flutter:extensions.bzl",
    _flutter = "flutter",
    _pub = "pub",
)

# Consumers use this file so their tags join the same extension instances as
# Razemify's tags. Re-exporting from any other file creates a different Bzlmod
# extension identity and incompatible Flutter provider/repository instances.
flutter = _flutter
pub = _pub
