// Bazel-only entrypoint: the stable URI keeps kernels independent of the
// sandbox path while the real Flutter/Gradle entrypoint remains nested.
import 'org-dartlang-bazel:///flutter_razemify/example/lib/main.dart' as app;

void main() => app.main();
