import 'package:local_auth/local_auth.dart';

class LocalAuthService {
  LocalAuthService._();

  static final LocalAuthService instance = LocalAuthService._();

  Future<bool> canAuthenticate() async {
    try {
      final auth = LocalAuthentication();
      final canBiometrics = await auth.canCheckBiometrics;
      final deviceSupported = await auth.isDeviceSupported();
      return canBiometrics || deviceSupported;
    } catch (_) {
      return false;
    }
  }

  Future<bool> authenticate(String reason) async {
    try {
      final auth = LocalAuthentication();
      return await auth.authenticate(
        localizedReason: reason,
        options: const AuthenticationOptions(
          biometricOnly: false,
          stickyAuth: true,
        ),
      );
    } catch (_) {
      return false;
    }
  }
}
