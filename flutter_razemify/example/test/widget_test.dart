// Smoke test for the example app's initial frame.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter/material.dart';
import 'package:flutter_razemify/flutter_razemify.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:flutter_razemify_example/main.dart';

void main() {
  testWidgets('renders the home page with preset and palette pickers', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(const MyApp());

    expect(find.text('Razemify Example'), findsOneWidget);
    expect(find.widgetWithText(ElevatedButton, 'Pick Image'), findsOneWidget);
    expect(find.byType(DropdownButtonFormField<Preset>), findsOneWidget);
    expect(find.byType(DropdownButtonFormField<Palette>), findsOneWidget);
  });
}
