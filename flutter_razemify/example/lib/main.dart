import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_razemify/flutter_razemify.dart';
import 'package:image_picker/image_picker.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Razemify Demo',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.red),
        useMaterial3: true,
      ),
      home: const MyHomePage(),
    );
  }
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({super.key});

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  Uint8List? _originalImage;
  Uint8List? _processedImage;
  bool _isProcessing = false;
  String? _processingTime;
  Preset _selectedPreset = Preset.detailedStandard;
  Palette _selectedPalette = Palette.original;

  final ImagePicker _picker = ImagePicker();

  Future<void> _pickImage() async {
    try {
      final XFile? image = await _picker.pickImage(source: ImageSource.gallery);
      if (image == null) return;

      final bytes = await image.readAsBytes();
      setState(() {
        _originalImage = bytes;
        _processedImage = null;
        _processingTime = null;
      });
    } catch (e) {
      _showError('Failed to pick image: $e');
    }
  }

  Future<void> _processImage() async {
    if (_originalImage == null) return;

    setState(() {
      _isProcessing = true;
      _processingTime = null;
    });

    try {
      final result = await Razemify.processImageWithDetails(
        _originalImage!,
        preset: _selectedPreset,
        palette: _selectedPalette,
      );

      final pngBytes = await result.toPng();

      setState(() {
        _processedImage = pngBytes;
        _processingTime =
            '${result.processingTime.inMilliseconds}ms (${result.width}x${result.height})';
      });
    } on RazemifyException catch (e) {
      _showError('Processing failed: $e');
    } catch (e) {
      _showError('Unexpected error: $e');
    } finally {
      setState(() => _isProcessing = false);
    }
  }

  void _showError(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(message), backgroundColor: Colors.red),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        title: const Text('Razemify Example'),
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Preset selector
            DropdownButtonFormField<Preset>(
              value: _selectedPreset,
              decoration: const InputDecoration(
                labelText: 'Preset',
                border: OutlineInputBorder(),
              ),
              items: Preset.values.map((preset) {
                return DropdownMenuItem(
                  value: preset,
                  child: Text(preset.value),
                );
              }).toList(),
              onChanged: (value) {
                if (value != null) {
                  setState(() => _selectedPreset = value);
                }
              },
            ),
            const SizedBox(height: 16),

            // Palette selector
            DropdownButtonFormField<Palette>(
              value: _selectedPalette,
              decoration: const InputDecoration(
                labelText: 'Palette',
                border: OutlineInputBorder(),
              ),
              items: Palette.values.map((palette) {
                return DropdownMenuItem(
                  value: palette,
                  child: Text(palette.value),
                );
              }).toList(),
              onChanged: (value) {
                if (value != null) {
                  setState(() => _selectedPalette = value);
                }
              },
            ),
            const SizedBox(height: 24),

            // Action buttons
            Row(
              children: [
                Expanded(
                  child: ElevatedButton.icon(
                    onPressed: _pickImage,
                    icon: const Icon(Icons.image),
                    label: const Text('Pick Image'),
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: ElevatedButton.icon(
                    onPressed: _originalImage != null && !_isProcessing
                        ? _processImage
                        : null,
                    icon: _isProcessing
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.play_arrow),
                    label: const Text('Process'),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 24),

            // Processing time
            if (_processingTime != null)
              Card(
                child: Padding(
                  padding: const EdgeInsets.all(8),
                  child: Text(
                    'Processing time: $_processingTime',
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                ),
              ),
            const SizedBox(height: 16),

            // Image display
            if (_originalImage != null)
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    'Original',
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(height: 8),
                  ClipRRect(
                    borderRadius: BorderRadius.circular(8),
                    child: Image.memory(_originalImage!, fit: BoxFit.contain),
                  ),
                  const SizedBox(height: 24),
                ],
              ),

            if (_processedImage != null)
              Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    'Processed',
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(height: 8),
                  ClipRRect(
                    borderRadius: BorderRadius.circular(8),
                    child: Image.memory(_processedImage!, fit: BoxFit.contain),
                  ),
                ],
              ),
          ],
        ),
      ),
    );
  }
}
