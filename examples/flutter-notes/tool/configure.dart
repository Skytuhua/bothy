// Writes oauth.json (for --dart-define-from-file) and ios/Flutter/OAuth.xcconfig from your
// OAuth clients. Both files are git-ignored.
//
//   dart run tool/configure.dart --desktop client_secret_xxx.json \
//       --ios-client-id 123-abc.apps.googleusercontent.com \
//       --web-client-id 123-def.apps.googleusercontent.com
import 'dart:convert';
import 'dart:io';

void main(List<String> args) {
  String? arg(String name) {
    final i = args.indexOf('--$name');
    return i >= 0 && i + 1 < args.length ? args[i + 1] : null;
  }

  final defines = <String, String>{};
  final desktop = arg('desktop');
  if (desktop != null) {
    final client = desktopClient(desktop);
    defines['GOOGLE_DESKTOP_CLIENT_ID'] = client.$1;
    defines['GOOGLE_DESKTOP_CLIENT_SECRET'] = client.$2;
  }
  final ios = arg('ios-client-id');
  if (ios != null) defines['GOOGLE_IOS_CLIENT_ID'] = clientId(ios, 'ios');
  final web = arg('web-client-id');
  if (web != null) defines['GOOGLE_WEB_CLIENT_ID'] = clientId(web, 'web');
  final local = arg('local-remote');
  if (local != null) defines['BOTHY_LOCAL_REMOTE'] = local;
  if (defines.isEmpty) {
    stderr.writeln(
      'usage: dart run tool/configure.dart [--desktop client.json] '
      '[--ios-client-id ID] [--web-client-id ID] [--local-remote DIR]',
    );
    exit(2);
  }

  final out = File('oauth.json');
  out.writeAsStringSync(const JsonEncoder.withIndent('  ').convert(defines));
  if (!Platform.isWindows) Process.runSync('chmod', ['600', out.path]);
  stdout.writeln('wrote oauth.json');

  if (ios != null) {
    // The iOS sign-in redirect uses the client id reversed as a URL scheme.
    final reversed = ios.split('.').reversed.join('.');
    File('ios/Flutter/OAuth.xcconfig')
        .writeAsStringSync('GOOGLE_REVERSED_CLIENT_ID=$reversed\n');
    stdout.writeln('wrote ios/Flutter/OAuth.xcconfig');
  }
}

Never fail(String message) {
  stderr.writeln('error: $message');
  exit(2);
}

/// A client id as the console shows it.
String clientId(String id, String what) {
  if (!id.endsWith('.apps.googleusercontent.com')) {
    fail('--$what-client-id must end in .apps.googleusercontent.com');
  }
  return id;
}

/// The id and secret of a "Desktop app" client file, with the same checks as
/// `bothy setup google`: the desktop client is the `installed` one with redirect URIs.
(String, String) desktopClient(String path) {
  final Object? json;
  try {
    json = jsonDecode(File(path).readAsStringSync());
  } on Object {
    fail('$path is not a client JSON file from the Cloud console');
  }
  if (json is! Map) fail('$path is not a client JSON file');
  if (json['web'] != null) {
    fail(
      '$path is a "Web application" client; --desktop needs a "Desktop app" client',
    );
  }
  final c = json['installed'];
  if (c is! Map) fail('$path is not an OAuth client file');
  final secret = c['client_secret'];
  if (secret is! String || secret.isEmpty) {
    fail(
      '$path is an Android or iOS client; --desktop needs a "Desktop app" client',
    );
  }
  if (c['redirect_uris'] == null) {
    fail(
      '$path is a "TVs and Limited Input devices" client; --desktop needs a "Desktop app" client',
    );
  }
  return (clientId(c['client_id'] as String? ?? '', 'desktop'), secret);
}
