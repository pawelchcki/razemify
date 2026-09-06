# Razemify WASM Usage Guide

## Quick Start

1. Build the WASM module:
```bash
cd /path/to/razemify
bazel run //xtask -- wasm build --release
```

2. Test locally:
```bash
python3 -m http.server 8080 --directory www
# Open http://localhost:8080 in your browser
```

## Integration

### In HTML

```html
<!DOCTYPE html>
<html>
<head>
    <title>Razemify Demo</title>
</head>
<body>
    <input type="file" id="input" accept="image/*">
    <button id="process">Process</button>
    <img id="output">

    <script type="module" src="app.js"></script>
</body>
</html>
```

### In JavaScript

```javascript
import init, { RazemifyProcessor, DetailedParams, ColorPalette } from './pkg/razemify_wasm.js';

// Initialize WASM (call once at startup)
await init();

// Handle file input
document.getElementById('input').addEventListener('change', async (e) => {
    const file = e.target.files[0];
    const imageBytes = new Uint8Array(await file.arrayBuffer());

    // Process with standard preset and original palette
    const params = DetailedParams.detailedStandard();
    const palette = ColorPalette.original();

    const resultBytes = RazemifyProcessor.processImage(
        imageBytes,
        params,
        palette
    );

    // Display result
    const blob = new Blob([resultBytes], { type: 'image/png' });
    const url = URL.createObjectURL(blob);
    document.getElementById('output').src = url;
});
```

## API Reference

### DetailedParams

Processing parameters that control the posterization algorithm.

#### Factory Methods

- `DetailedParams.detailedStandard()` - Balanced quality and detail (recommended)
- `DetailedParams.detailedStrong()` - Enhanced contrast and sharpness
- `DetailedParams.detailedFine()` - Fine detail preservation

#### Constructor

```javascript
new DetailedParams(threshLow, threshHigh, clipLimit, tileSize)
```

Parameters:
- `threshLow` (u8): Lower threshold for background/midtone separation (0-255)
- `threshHigh` (u8): Upper threshold for midtone/highlight separation (0-255)
- `clipLimit` (f64): CLAHE clip limit for contrast enhancement (1.0-10.0)
- `tileSize` (u32): CLAHE tile size for local contrast (4-16)

Examples:
```javascript
// Standard preset equivalent
const params = new DetailedParams(80, 160, 3.0, 8);

// High contrast
const params = new DetailedParams(70, 150, 4.0, 8);

// Fine detail
const params = new DetailedParams(80, 160, 2.5, 4);
```

### ColorPalette

Color scheme for the posterized output (3 colors: background, midtone, highlight).

#### Factory Methods

- `ColorPalette.original()` - Black, deep pink, gold (default)
- `ColorPalette.burgundy()` - Black, dark burgundy, white
- `ColorPalette.burgundyTeal()` - Dark burgundy, burgundy, teal
- `ColorPalette.burgundyGold()` - Black, deep burgundy, warm gold
- `ColorPalette.rose()` - Deep burgundy, rose, light pink
- `ColorPalette.cmyk()` - Black, cyan, magenta

#### Constructor

```javascript
new ColorPalette(bgHex, midtoneHex, highlightHex)
```

Parameters:
- `bgHex` (string): Background color as hex string (e.g., "#000000" or "000000")
- `midtoneHex` (string): Midtone color as hex string
- `highlightHex` (string): Highlight color as hex string

Examples:
```javascript
// Custom purple/gold scheme
const palette = new ColorPalette("4A0E4E", "9B59B6", "FFD700");

// Blue monochrome
const palette = new ColorPalette("001F3F", "0074D9", "7FDBFF");
```

### RazemifyProcessor

Static class for image processing.

#### Methods

```javascript
RazemifyProcessor.processImage(imageBytes, params, palette)
```

Parameters:
- `imageBytes` (Uint8Array): Input image bytes (PNG, JPEG, WebP, BMP, etc.)
- `params` (DetailedParams): Processing parameters
- `palette` (ColorPalette): Color palette to apply

Returns:
- `Uint8Array`: PNG-encoded result image bytes

Example:
```javascript
const imageBytes = new Uint8Array(await file.arrayBuffer());
const params = DetailedParams.detailedStandard();
const palette = ColorPalette.burgundy();

const resultBytes = RazemifyProcessor.processImage(
    imageBytes,
    params,
    palette
);

// Save or display result
const blob = new Blob([resultBytes], { type: 'image/png' });
const url = URL.createObjectURL(blob);
```

## Image Format Support

### Input Formats
- PNG (with or without alpha channel)
- JPEG
- WebP
- BMP
- And more formats supported by the `image` crate

### Output Format
- PNG (always, with preserved alpha channel if present)

### Alpha Channel Handling
- If input has alpha channel: preserved in output
- If input has no alpha: assumes fully opaque (alpha = 255)
- Pixels with alpha < 10 are rendered as background color

## Performance

Typical processing times (WASM in Chrome on modern desktop):

| Image Size | Standard Preset | Strong Preset |
|------------|----------------|---------------|
| 512x512    | ~200ms         | ~250ms        |
| 1024x1024  | ~800ms         | ~1s           |
| 2048x2048  | ~3s            | ~4s           |

Performance varies by browser and device. Mobile devices will be slower.

## Bundle Size

- WASM binary: ~2.4MB (uncompressed)
- JavaScript glue: ~16KB
- Total download: ~2.4MB

The bundle is larger than initially estimated because it includes:
- Full image format decoders (PNG, JPEG, WebP, etc.)
- CLAHE implementation
- Edge detection and posterization algorithms

### Size Optimization Tips

1. Use gzip/brotli compression on your web server
2. Lazy-load the WASM module only when needed
3. Consider code splitting if using a bundler

## Browser Compatibility

Requires:
- WebAssembly support (all modern browsers)
- ES6 modules support
- Async/await support

Tested on:
- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

## Troubleshooting

### "Failed to load WASM module"
- Ensure you're serving files over HTTP/HTTPS (not file://)
- Check MIME types are correct (.wasm should be application/wasm)
- Check browser console for CORS errors

### "Out of memory" errors
- Large images (>4000x4000) may exceed WASM memory limits
- Try resizing images before processing
- Close other tabs to free up memory

### Slow performance
- Processing is CPU-intensive
- Consider showing a loading indicator
- For batch processing, use Web Workers

### Module not found errors
- Check the import path points to the correct pkg/ directory
- Ensure the Bazel WASM build completed successfully
- Check file permissions on generated files

## Examples

### Basic Processing

```javascript
import init, { RazemifyProcessor, DetailedParams, ColorPalette } from './pkg/razemify_wasm.js';

await init();

async function processImage(file) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const result = RazemifyProcessor.processImage(
        bytes,
        DetailedParams.detailedStandard(),
        ColorPalette.original()
    );

    return new Blob([result], { type: 'image/png' });
}
```

### With Progress Indicator

```javascript
async function processWithProgress(file, progressCallback) {
    progressCallback('Loading image...');
    const bytes = new Uint8Array(await file.arrayBuffer());

    progressCallback('Processing...');
    const result = RazemifyProcessor.processImage(
        bytes,
        DetailedParams.detailedStandard(),
        ColorPalette.original()
    );

    progressCallback('Done!');
    return new Blob([result], { type: 'image/png' });
}
```

### Batch Processing with Web Worker

```javascript
// worker.js
import init, { RazemifyProcessor, DetailedParams, ColorPalette } from './pkg/razemify_wasm.js';

await init();

self.onmessage = async (e) => {
    const { imageBytes, presetName, paletteName } = e.data;

    const params = DetailedParams[presetName]();
    const palette = ColorPalette[paletteName]();

    const result = RazemifyProcessor.processImage(imageBytes, params, palette);

    self.postMessage({ result }, [result.buffer]);
};
```

```javascript
// main.js
const worker = new Worker('worker.js', { type: 'module' });

worker.onmessage = (e) => {
    const blob = new Blob([e.data.result], { type: 'image/png' });
    // Display or download result
};

worker.postMessage({
    imageBytes: imageData,
    presetName: 'detailedStandard',
    paletteName: 'original'
});
```

## Advanced Usage

### Custom Parameters

Fine-tune the processing for specific use cases:

```javascript
// High contrast for text/logos
const params = new DetailedParams(60, 180, 5.0, 8);

// Subtle effect for photos
const params = new DetailedParams(90, 140, 2.0, 4);

// Maximum detail
const params = new DetailedParams(80, 160, 2.0, 2);
```

### Multiple Palettes

Process the same image with different color schemes:

```javascript
const palettes = [
    ColorPalette.original(),
    ColorPalette.burgundy(),
    ColorPalette.rose(),
    ColorPalette.cmyk()
];

const results = palettes.map(palette =>
    RazemifyProcessor.processImage(imageBytes, params, palette)
);
```

### Error Handling

```javascript
try {
    const result = RazemifyProcessor.processImage(imageBytes, params, palette);
    // Success
} catch (error) {
    if (error.message.includes('Failed to load image')) {
        console.error('Invalid image format');
    } else if (error.message.includes('Out of memory')) {
        console.error('Image too large');
    } else {
        console.error('Processing failed:', error);
    }
}
```
