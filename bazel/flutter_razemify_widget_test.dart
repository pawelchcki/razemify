import 'package:flutter/material.dart';
import 'package:flutter_razemify/flutter_razemify.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('renders a Razemify preset in a Flutter widget', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(home: Text(Preset.detailedStandard.value)),
    );

    expect(find.text(Preset.detailedStandard.value), findsOneWidget);
  });
}
