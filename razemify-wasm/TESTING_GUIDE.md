# Testing Guide: ONNX Background Removal

This guide walks through testing the new background removal feature.

## Quick Start

1. **Build the WASM package**:
   ```bash
   cd /path/to/razemify
   bazel run //xtask -- wasm build --release
   ```

2. **Start a local server**:
   ```bash
   # Option 1: Python
   python3 -m http.server 8000 --directory razemify-wasm

   # Option 2: Node.js
   npx http-server razemify-wasm -p 8000

   # Option 3: Wrangler (Cloudflare)
   wrangler pages dev razemify-wasm
   ```

3. **Open in browser**:
   ```
   http://localhost:8000/www/
   ```

## Test Plan

### Phase 1: Basic Functionality

#### Test 1.1: Enable Background Removal
1. Open app in Chrome 113+ (for WebGPU support)
2. Check "Enable Background Removal" checkbox
3. ✅ Verify controls section appears
4. ✅ Verify model status shows "No model loaded"

#### Test 1.2: Upload U2Net Model
1. Download U2Net model:
   - URL: https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2net.onnx
   - Size: ~176 MB
2. Select "Upload Model File" from Model Source
3. Click "Choose File" and select u2net.onnx
4. ✅ Verify progress bar shows loading
5. ✅ Verify model type auto-detected as "u2net"
6. ✅ Verify model status shows "Model loaded: U2NET (320x320)"
7. ✅ Verify status background turns green

#### Test 1.3: Process Image with U2Net
1. Upload a test image (person with clear background)
2. ✅ Verify progress bar shows "Removing background..."
3. ✅ Verify progress shows "Preprocessing", "Running inference", "Applying mask"
4. ✅ Verify result shows posterized image with transparent background
5. ✅ Verify processing completes in < 5 seconds (WebGPU)
6. ✅ Verify checkerboard pattern visible through transparent areas

### Phase 2: Model Caching

#### Test 2.1: Verify Model Caching
1. After loading model in Test 1.2, reload page (F5)
2. Check "Enable Background Removal"
3. Upload same model file again
4. ✅ Verify loading completes in < 1 second (cached)
5. ✅ Verify progress shows "Loading from cache..."

#### Test 2.2: Clear Cache and Reload
1. Open browser DevTools → Application → Storage
2. Delete IndexedDB → razemify_models
3. Close DevTools
4. Upload model again
5. ✅ Verify full loading process (not cached)
6. ✅ Verify model re-cached for next time

### Phase 3: CDN Model Loading

#### Test 3.1: Load U2Net from CDN
1. Reload page
2. Check "Enable Background Removal"
3. Select "CDN: U2Net Lite" from Model Source
4. ✅ Verify download starts automatically
5. ✅ Verify progress shows download progress with MB counter
6. ✅ Verify model type set to "u2net"
7. ✅ Verify model loads successfully
8. ✅ Verify download takes 3-10 seconds (depending on connection)

#### Test 3.2: Verify CDN Cache
1. Reload page after Test 3.1
2. Select "CDN: U2Net Lite" again
3. ✅ Verify loads from cache (< 1 second)

### Phase 4: Custom URL Loading

#### Test 4.1: Load from Custom URL
1. Find a valid ONNX model URL (e.g., Hugging Face)
2. Select "Custom URL" from Model Source
3. Enter URL in text field
4. Click outside field to trigger load
5. ✅ Verify download starts
6. ✅ Verify progress tracking
7. ✅ Verify model loads successfully

### Phase 5: Model Type Testing

#### Test 5.1: BiRefNet Model
1. Download BiRefNet model (if available)
2. Upload via file input
3. Select "birefnet" from Model Type dropdown
4. Upload test image
5. ✅ Verify preprocessing at 1024x1024
6. ✅ Verify ImageNet normalization applied
7. ✅ Verify min-max normalization on output
8. ✅ Verify high-quality mask
9. ✅ Verify processing time < 5 seconds (WebGPU)

#### Test 5.2: ISNet Model
1. Download ISNet model (if available)
2. Upload via file input
3. Select "isnet" from Model Type dropdown
4. Upload test image
5. ✅ Verify preprocessing at 1024x1024
6. ✅ Verify ImageNet normalization applied
7. ✅ Verify sigmoid-only output processing
8. ✅ Verify good quality mask
9. ✅ Verify processing time < 3 seconds (WebGPU)

### Phase 6: Integration with Posterization

#### Test 6.1: Background Removal + Posterization
1. Load model
2. Upload image
3. Enable background removal
4. ✅ Verify transparent background
5. Change color palette
6. ✅ Verify re-processes correctly with new palette
7. ✅ Verify alpha channel preserved
8. Adjust sliders
9. ✅ Verify posterization updates
10. ✅ Verify background stays transparent

#### Test 6.2: Toggle Background Removal
1. Process image with background removal enabled
2. Uncheck "Enable Background Removal"
3. ✅ Verify controls hide
4. ✅ Verify auto-reprocessing without background removal
5. ✅ Verify background no longer transparent
6. Re-enable background removal
7. ✅ Verify re-processes with transparent background

### Phase 7: Auto-Process Mode

#### Test 7.1: Auto-Process with Background Removal
1. Enable background removal
2. Load model
3. Upload image
4. Check "Auto-process" checkbox
5. Adjust "Edge Detection (Low Threshold)" slider
6. ✅ Verify debounced auto-processing (500ms delay)
7. ✅ Verify background removal runs on each change
8. Adjust multiple sliders quickly
9. ✅ Verify only one processing call after settling

### Phase 8: Error Handling

#### Test 8.1: Invalid Model File
1. Upload non-ONNX file (.txt, .jpg, etc.)
2. ✅ Verify error message shown
3. ✅ Verify model status shows error (red background)

#### Test 8.2: Network Error (CDN)
1. Disconnect internet
2. Select "CDN: U2Net Lite"
3. ✅ Verify error message shown
4. ✅ Verify graceful failure

#### Test 8.3: Wrong Model Type
1. Upload U2Net model
2. Manually select "birefnet" from Model Type
3. Process image
4. ✅ Verify output (may be incorrect but shouldn't crash)

#### Test 8.4: No Image Loaded
1. Enable background removal
2. Load model
3. Try to process without loading image
4. ✅ Verify error message "No image loaded"

### Phase 9: Browser Compatibility

#### Test 9.1: Chrome (WebGPU)
1. Open in Chrome 113+
2. Run all Phase 1-3 tests
3. ✅ Verify WebGPU backend used (check console)
4. ✅ Verify fast inference (< 2s for U2Net)

#### Test 9.2: Firefox (WASM Backend)
1. Open in Firefox 90+
2. Run all Phase 1-3 tests
3. ✅ Verify WASM backend used (check console)
4. ✅ Verify slower inference (2-5s for U2Net)
5. ✅ Verify correct results

#### Test 9.3: Safari (WASM Backend)
1. Open in Safari 15+
2. Run all Phase 1-3 tests
3. ✅ Verify WASM backend used
4. ✅ Verify correct results

#### Test 9.4: Edge (WebGPU)
1. Open in Edge 113+
2. Run all Phase 1-3 tests
3. ✅ Verify WebGPU backend used
4. ✅ Verify fast inference

### Phase 10: Performance Testing

#### Test 10.1: Measure Inference Time
1. Load U2Net model
2. Upload test image
3. Open browser DevTools → Console
4. Note time from "Running inference..." to "Applying mask..."
5. ✅ Verify < 2 seconds (WebGPU)
6. ✅ Verify < 5 seconds (WASM backend)

#### Test 10.2: Measure Total Processing Time
1. Start timer when clicking image upload
2. Stop timer when result appears
3. ✅ Verify total time < 7 seconds (WebGPU)
4. ✅ Verify total time < 10 seconds (WASM backend)

#### Test 10.3: Large Image Performance
1. Upload 4K image (3840x2160)
2. Enable background removal
3. ✅ Verify resizing works correctly
4. ✅ Verify inference time similar (resolution normalized)
5. ✅ Verify mask resized back to 4K correctly

### Phase 11: UI/UX Testing

#### Test 11.1: Progress Indicators
1. Load large model from CDN
2. ✅ Verify progress bar visible
3. ✅ Verify progress text updates
4. ✅ Verify percentage increases
5. ✅ Verify MB downloaded shown
6. ✅ Verify progress hides when complete

#### Test 11.2: Status Messages
1. Perform various operations
2. ✅ Verify status messages clear after success (2s timeout)
3. ✅ Verify error messages persist
4. ✅ Verify color coding (blue=info, green=success, red=error)

#### Test 11.3: Download Result
1. Process image with transparent background
2. Click "Download Result" link
3. ✅ Verify downloads as PNG
4. ✅ Verify transparency preserved in downloaded file
5. Open in image editor
6. ✅ Verify alpha channel correct

## Automated Testing (Future)

### Unit Tests
```javascript
// Example tests to implement
describe('ONNX Background Removal', () => {
  test('detectModelType detects U2Net', () => {
    expect(detectModelType('u2net.onnx')).toBe('u2net');
  });

  test('getModelInputSize returns correct sizes', () => {
    expect(getModelInputSize('u2net')).toBe(320);
    expect(getModelInputSize('birefnet')).toBe(1024);
  });

  test('sigmoid function works correctly', () => {
    expect(1.0 / (1.0 + Math.exp(-0))).toBeCloseTo(0.5);
  });
});
```

### Integration Tests
- Test full pipeline with mock ONNX session
- Test caching layer with mock IndexedDB
- Test error handling with mock failures

## Performance Benchmarks

Record these metrics across browsers:

| Metric | Chrome (WebGPU) | Firefox (WASM) | Safari (WASM) | Edge (WebGPU) |
|--------|----------------|----------------|---------------|---------------|
| Model load (first) | | | | |
| Model load (cached) | | | | |
| Preprocess (320x320) | | | | |
| Inference (U2Net) | | | | |
| Postprocess | | | | |
| Total pipeline | | | | |

## Common Issues and Solutions

### Issue: WebGPU not available
**Solution**: Update browser to Chrome/Edge 113+ or use Firefox/Safari (WASM backend)

### Issue: Model download fails
**Solution**: Check internet connection, try different CDN URL, or upload local file

### Issue: Out of memory
**Solution**: Try smaller model (U2Net instead of BiRefNet), close other tabs

### Issue: IndexedDB quota exceeded
**Solution**: Clear browser data, delete other cached models

### Issue: Slow performance
**Solution**: Use Chrome/Edge with WebGPU, close background tabs, try smaller images

## Test Images

Recommended test images:
1. **Portrait**: Person with simple background
2. **Product**: Object on white/solid background
3. **Complex**: Person with detailed hair, complex background
4. **High-res**: 4K image (3840x2160)
5. **Transparent**: Already has transparency (RGBA PNG)

## Test Models

Download these models for testing:

1. **U2Net** (~176 MB):
   - https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2net.onnx

2. **U2Net Lite** (~4.7 MB):
   - https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx

3. **BiRefNet** (if available):
   - Search Hugging Face for BiRefNet ONNX models

4. **ISNet** (if available):
   - https://github.com/xuebinqin/DIS (check releases)

## Reporting Issues

When reporting issues, include:
- Browser name and version
- Model type and size
- Image dimensions
- Error message (exact text)
- Console errors (F12 → Console)
- Steps to reproduce
- Expected vs actual behavior

## Success Criteria

All tests should pass with:
- ✅ No JavaScript errors in console
- ✅ Processing completes successfully
- ✅ Transparent background visible (checkerboard pattern)
- ✅ Posterization applied correctly
- ✅ Alpha channel preserved in downloads
- ✅ Performance within expected ranges
- ✅ UI responsive during processing
- ✅ Error messages clear and helpful
