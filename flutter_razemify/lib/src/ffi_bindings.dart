import 'dart:ffi' as ffi;
import 'dart:io';
import 'package:ffi/ffi.dart';
import 'models.dart';

/// FFI function signatures matching Rust implementation
typedef ProcessBytesNative =
    ffi.Int32 Function(
      ffi.Pointer<ffi.Uint8> inputData,
      ffi.Size inputLen,
      ffi.Pointer<ffi.Uint8> outputData,
      ffi.Pointer<ffi.Uint32> outputWidth,
      ffi.Pointer<ffi.Uint32> outputHeight,
      ffi.Pointer<Utf8> presetName,
      ffi.Pointer<Utf8> paletteName,
    );

typedef ProcessBytesDart =
    int Function(
      ffi.Pointer<ffi.Uint8> inputData,
      int inputLen,
      ffi.Pointer<ffi.Uint8> outputData,
      ffi.Pointer<ffi.Uint32> outputWidth,
      ffi.Pointer<ffi.Uint32> outputHeight,
      ffi.Pointer<Utf8> presetName,
      ffi.Pointer<Utf8> paletteName,
    );

typedef GetOutputSizeNative =
    ffi.Int32 Function(
      ffi.Pointer<ffi.Uint8> inputData,
      ffi.Size inputLen,
      ffi.Pointer<ffi.Uint32> widthOut,
      ffi.Pointer<ffi.Uint32> heightOut,
    );

typedef GetOutputSizeDart =
    int Function(
      ffi.Pointer<ffi.Uint8> inputData,
      int inputLen,
      ffi.Pointer<ffi.Uint32> widthOut,
      ffi.Pointer<ffi.Uint32> heightOut,
    );

// Note: c_char in Rust is i8 (signed), not u8
typedef FreeStringNative = ffi.Void Function(ffi.Pointer<ffi.Int8>);
typedef FreeStringDart = void Function(ffi.Pointer<ffi.Int8>);

/// Load the native library with improved error handling
ffi.DynamicLibrary _loadLibrary() {
  try {
    late ffi.DynamicLibrary lib;

    if (Platform.isAndroid) {
      try {
        lib = ffi.DynamicLibrary.open('libflutter_razemify.so');
      } catch (e) {
        throw RazemifyException(
          'Failed to load native library on Android. '
          'This usually means the Rust library was not built or packaged correctly. '
          'Error: $e',
        );
      }
    } else if (Platform.isIOS || Platform.isMacOS) {
      try {
        lib = ffi.DynamicLibrary.process();
      } catch (e) {
        throw RazemifyException(
          'Failed to load native library on iOS/macOS. '
          'Ensure the Rust library is properly linked in the app bundle. '
          'Error: $e',
        );
      }
    } else if (Platform.isLinux) {
      try {
        lib = ffi.DynamicLibrary.open('libflutter_razemify.so');
      } catch (e) {
        throw RazemifyException(
          'Failed to load native library on Linux. '
          'Check that libflutter_razemify.so is in the library path. '
          'Error: $e',
        );
      }
    } else {
      throw UnsupportedError(
        'Razemify is not supported on ${Platform.operatingSystem}. '
        'Currently supported platforms: Android, iOS, macOS, Linux.',
      );
    }

    // Verify critical symbols exist
    try {
      lib.lookup<ffi.NativeFunction<ProcessBytesNative>>(
        'razemify_process_bytes',
      );
      lib.lookup<ffi.NativeFunction<GetOutputSizeNative>>(
        'razemify_get_output_size',
      );
      lib.lookup<ffi.NativeFunction<FreeStringNative>>('razemify_free_string');
    } catch (e) {
      throw RazemifyException(
        'Native library loaded but is missing required functions. '
        'This indicates an incomplete or incompatible library build. '
        'Error: $e',
      );
    }

    return lib;
  } catch (e) {
    if (e is RazemifyException || e is UnsupportedError) {
      rethrow;
    }
    throw RazemifyException('Unexpected error loading native library: $e');
  }
}

/// FFI bindings to the native Razemify library
class RazemifyBindings {
  static final ffi.DynamicLibrary _lib = _loadLibrary();

  static final ProcessBytesDart processBytes = _lib
      .lookup<ffi.NativeFunction<ProcessBytesNative>>('razemify_process_bytes')
      .asFunction();

  static final GetOutputSizeDart getOutputSize = _lib
      .lookup<ffi.NativeFunction<GetOutputSizeNative>>(
        'razemify_get_output_size',
      )
      .asFunction();

  static final FreeStringDart freeString = _lib
      .lookup<ffi.NativeFunction<FreeStringNative>>('razemify_free_string')
      .asFunction();
}
