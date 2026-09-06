# Quick Start: ONNX Background Removal

Get started with background removal in 5 minutes.

## 1. Build the WASM Package

```bash
cd /path/to/razemify
bazel run //xtask -- wasm build --release
```

## 2. Start Local Server

```bash
# Simple Python server
python3 -m http.server 8000 --directory razemify-wasm/www
```

## 3. Open in Browser

Navigate to: http://localhost:8000/

## 4. Enable Background Removal

1. Check the "Enable Background Removal" checkbox
2. Controls section will appear

## 5. Load a Model

### Option A: CDN (Easiest)
1. Select "CDN: U2Net Lite" from Model Source dropdown
2. Model downloads automatically (~4.7 MB)
3. Wait for "Model loaded" message

### Option B: Bundle Local Model (Recommended)
1. Download model with xtask:
   ```bash
   cargo xtask models download birefnet-lite
   cargo xtask models bundle birefnet-lite
   ```
   See `../xtask/README.md` for details.

2. Select "Bundled: BiRefNet Lite" from Model Source
3. Model loads instantly from local cache

### Option C: Upload File
1. Select "Upload Model File" from Model Source
2. Click "Choose File" and select .onnx model
3. Wait for "Model loaded" message

## 6. Process an Image

1. Click "Select Image" and choose a photo
2. Wait for processing to complete
3. Result shows with transparent background
4. Click "Download Result" to save

## 7. Try Different Settings

- Change color palette
- Adjust edge detection sliders
- Toggle background removal on/off
- Try different models (BiRefNet, ISNet)

## That's It!

You now have browser-based background removal + posterization.

## Troubleshooting

**Model won't load?**
- Check internet connection (for CDN)
- Verify .onnx file format (for upload)
- Check browser console (F12) for errors

**Slow performance?**
- Use Chrome 113+ or Edge 113+ for WebGPU
- Try smaller model (U2Net instead of BiRefNet)
- Disable auto-process mode

**Background not removed well?**
- Try different model (BiRefNet for quality)
- Ensure high contrast between subject and background
- Check model type is correctly detected

## Next Steps

- Read `ONNX_BACKGROUND_REMOVAL.md` for full documentation
- Follow `TESTING_GUIDE.md` for comprehensive testing
- Check `IMPLEMENTATION_NOTES.md` for technical details

## Model Recommendations

**For Speed**: U2Net Lite (4.7 MB, 0.5-2s inference)
**For Quality**: BiRefNet (200 MB, 2-5s inference)
**Balanced**: ISNet (43 MB, 1-3s inference)

## Browser Requirements

- Chrome/Edge 113+ (WebGPU - fastest)
- Firefox 90+ (WASM - slower but works)
- Safari 15+ (WASM - slower but works)

## File Locations

- **HTML**: `www/index.html`
- **JavaScript**: `www/index.js`
- **Documentation**: `*.md` files in this directory
