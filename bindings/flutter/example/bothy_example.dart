import 'package:bothy/bothy.dart';

void main() {
  final hlc = Hlc.parse('018bcfe568000001');
  print(
      'format v$bothyFormatVersion, physical=${hlc.physicalMs} logical=${hlc.logical}');
}
