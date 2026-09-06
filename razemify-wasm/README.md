# razemify-wasm

WebAssembly bindings for the razemify image processing library.

## Features

- Posterize images with detailed algorithm
- Preserve alpha channel transparency
- Browser-friendly API
- Bazel-built web package with hermetic Rust and wasm-bindgen toolchains

## Building

```bash
# Build for web with the hermetic Bazel toolchain
bazel run //xtask -- wasm build --release

# Test locally
python3 -m http.server 8080 --directory www
```

## Deployment

Deploy to Cloudflare Pages:

```bash
# Build and deploy in one command
./build-and-deploy.sh

# Or deploy only (if already built)
./deploy-to-cloudflare.sh
```

See [DEPLOYMENT.md](DEPLOYMENT.md) for detailed deployment instructions and troubleshooting.

## Usage

```javascript
import init, { RazemifyProcessor, DetailedParams, ColorPalette } from './pkg/razemify_wasm.js';

// Initialize WASM module
await init();

// Load image
const file = document.getElementById('input').files[0];
const imageBytes = new Uint8Array(await file.arrayBuffer());

// Process image
const params = DetailedParams.detailedStandard();
const palette = ColorPalette.original();
const resultBytes = RazemifyProcessor.processImage(imageBytes, params, palette);

// Display result
const blob = new Blob([resultBytes], { type: 'image/png' });
document.getElementById('output').src = URL.createObjectURL(blob);
```

## API

### DetailedParams

Processing parameters for the detailed posterization algorithm.

**Factory methods:**
- `DetailedParams.detailedStandard()` - Balanced quality and detail
- `DetailedParams.detailedStrong()` - Enhanced contrast and sharpness
- `DetailedParams.detailedFine()` - Fine detail preservation

**Constructor:**
- `new DetailedParams(threshLow, threshHigh, clipLimit, tileSize)`

### ColorPalette

Color schemes for posterization output.

**Presets:**
- `ColorPalette.original()` - Burgundy/cream/orange (default)
- `ColorPalette.burgundy()` - Same as original
- `ColorPalette.teal()` - Teal/cream/gold
- `ColorPalette.royal()` - Navy/cream/gold
- And more...

**Constructor:**
- `new ColorPalette(bgHex, midtoneHex, highlightHex)` - Custom colors

### RazemifyProcessor

Static processing methods.

**Methods:**
- `RazemifyProcessor.processImage(imageBytes, params, palette)` - Process image and return PNG bytes

## Image Handling

### Input
- Accepts any format: PNG, JPEG, WebP, BMP
- Load from File API: `file.arrayBuffer()` → `Uint8Array`
- Alpha channel preserved if present, otherwise assumes fully opaque

### Output
- Returns PNG-encoded bytes as `Uint8Array`
- Create Blob: `new Blob([resultBytes], { type: 'image/png' })`
- Display via Object URL or download

## Bundle Size

- Optimized WASM binary: approximately 5 MB
- JavaScript glue: approximately 16 KB
- Exact size varies with dependencies and the selected Bazel compilation mode

## License

Same as parent project.
