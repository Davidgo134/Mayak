import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';

import '../../core/config/build_profile.dart';
import '../screens/profile/spoof_screen.dart';

/// «Подмена данных» в разделе «Для разработчиков»
/// (переехала из основных настроек).
class DebugSpoofSection extends StatelessWidget {
  const DebugSpoofSection({super.key});

  @override
  Widget build(BuildContext context) {
    if (!BuildProfile.spoofUi) return const SizedBox.shrink();
    final cs = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
      child: Material(
        color: cs.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(20),
        child: InkWell(
          borderRadius: BorderRadius.circular(20),
          onTap: () => Navigator.of(context).push(
            MaterialPageRoute(builder: (_) => const SpoofScreen()),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 17),
            child: Row(
              children: [
                Icon(
                  Symbols.shield_lock,
                  color: cs.onSurfaceVariant,
                  size: 22,
                  weight: 400,
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Подмена данных',
                        style: TextStyle(
                          color: cs.onSurface,
                          fontSize: 16,
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        'Подмена устройства и данных сессии',
                        style: TextStyle(
                          color: cs.onSurfaceVariant,
                          fontSize: 13,
                        ),
                      ),
                    ],
                  ),
                ),
                Icon(
                  Symbols.chevron_right,
                  color: cs.onSurfaceVariant,
                  size: 22,
                  weight: 400,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
