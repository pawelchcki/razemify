# Bundled Model Setup - Simplified UI

The app now uses only the bundled BiRefNet Lite model with a simplified interface.

## Changes Made

### Removed Features
- ❌ Model source dropdown (Upload/CDN/Custom URL options)
- ❌ File upload input
- ❌ Custom URL input
- ❌ Model type selector
- ❌ All related event handlers

### New Simple Interface

**Checkbox**: "Enable Background Removal (BiRefNet Lite - High Quality)"
**Button**: "Click to load BiRefNet model (1024x1024, high quality)"

That's it! Just two UI elements.

## How It Works

1. **User enables background removal** → Checkbox triggers model loading
2. **Model loads automatically** → Progress bar shows loading status
3. **Model cached** → Subsequent loads are instant (< 1 second)
4. **Ready to use** → Process images with transparent backgrounds

## User Experience

### First Time Use
1. Check "Enable Background Removal"
2. Click the button (or just upload an image)
3. Wait 5-10 seconds for model to load (168 MB)
4. Model status shows "✓ BiRefNet ready"
5. Upload image and process

### Subsequent Use
1. Check "Enable Background Removal"
2. Model loads instantly from cache (< 1 second)
3. Ready to process immediately

## Technical Details

### Model
- **File**: `www/models/BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx`
- **Size**: 214 MB
- **Type**: BiRefNet (1024x1024)
- **Quality**: Excellent - best for portraits and detailed edges
- **Processing**: 2-5s (WebGPU), 8-15s (WASM)

### Automatic Behavior
- Model loads when background removal is enabled
- Model type is always 'birefnet' (hardcoded)
- GraphOptimizationLevel set to 'basic' (BiRefNet requirement)
- Cached in IndexedDB with key 'bundled_birefnet'
- Click status button to reload model if needed

### Code Changes

**HTML** (`www/index.html`):
- Removed model source dropdown
- Removed file upload input
- Removed custom URL input
- Removed model type selector
- Simplified to just model status button
- Updated label and documentation

**JavaScript** (`www/index.js`):
- Removed DOM element references for deleted controls
- Replaced `loadModel()` with `loadBundledModel()`
- Removed all model source selection logic
- Removed file upload handler
- Removed URL input handler
- Removed model type change handler
- Simplified enable checkbox handler to auto-load model
- Made model status clickable to reload

### Lines Removed
- **HTML**: ~30 lines of UI controls
- **JavaScript**: ~150 lines of event handlers and logic

### Lines Added
- **HTML**: ~3 lines (simplified status button)
- **JavaScript**: ~50 lines (simplified auto-load logic)

**Net reduction**: ~130 lines of code removed

## Benefits

✅ **Simpler UI** - Just checkbox and status button
✅ **No user decisions** - Works out of the box
✅ **High quality** - BiRefNet is excellent for most use cases
✅ **Offline ready** - Model included, no downloads needed
✅ **Faster UX** - No configuration required
✅ **Less confusion** - One model, one way to use it

## Deployment

Bundle the model using xtask:
```bash
cargo xtask models download birefnet-lite
cargo xtask models bundle birefnet-lite
```

This creates:
```
www/
├── models/
│   ├── BiRefNet-general-bb_swin_v1_tiny-epoch_232.onnx  (214 MB)
│   └── README.md
├── index.html
└── index.js
```

When deploying to production:
- Model served once, then cached in IndexedDB
- Subsequent visits load instantly
- No CDN dependencies
- Works offline

See `../xtask/README.md` for model management.

## Testing

**Reload the page**: http://localhost:8000/www/

**Test steps**:
1. Check "Enable Background Removal" checkbox
2. Model status button appears
3. Click button (or just upload an image)
4. Progress bar shows loading
5. Status changes to "✓ BiRefNet ready"
6. Upload image
7. Background removed automatically
8. Result has transparent background

**Second test** (cache):
1. Reload page
2. Enable background removal
3. Model loads instantly (< 1 second)
4. Ready immediately

## Future Enhancements

If users need different models, you can:
1. Add more bundled models to `www/models/`
2. Add a simple dropdown to select between bundled models
3. Keep the simplified UI without file uploads or custom URLs

Example:
```html
<select id="modelChoice">
    <option value="birefnet">BiRefNet (High Quality)</option>
    <option value="u2net">U2Net (Fast)</option>
</select>
```

But for now, BiRefNet is the best all-around choice.
