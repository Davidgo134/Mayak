import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';

import '../../widgets/connection_status.dart';

import '../../../core/config/build_profile.dart';
import '../../../core/config/mayak_settings.dart';
import '../../../main.dart';
import '../../widgets/section_header.dart';
import '../../widgets/settings_card.dart';

class MayakSettingsScreen extends StatelessWidget {
  const MayakSettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;

    return Scaffold(
      backgroundColor: cs.surface,
      appBar: ConnectionTitleBar(
        titleText: 'Маяк',
        backgroundColor: cs.surface,
      ),
      body: SafeArea(
        top: false,
        child: ListView(
          physics: const BouncingScrollPhysics(),
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 120),
          children: [
            const SectionHeader(
              'Сообщения',
              padding: EdgeInsets.fromLTRB(8, 0, 8, 8),
              fontSize: 14,
            ),
            SettingsCard(
              children: [
                if (BuildProfile.hiddenContentViewers) ...[
                  ValueListenableBuilder<bool>(
                    valueListenable: MayakSettings.viewDeleted,
                    builder: (context, value, _) => SettingsToggleTile(
                      icon: Symbols.delete_history,
                      label: 'View deleted message',
                      subtitle: 'Показывать удалённые сообщения',
                      value: value,
                      onChanged: MayakSettings.setViewDeleted,
                    ),
                  ),
                  ValueListenableBuilder<bool>(
                    valueListenable: MayakSettings.viewRedacted,
                    builder: (context, value, _) => SettingsToggleTile(
                      icon: Symbols.history_edu,
                      label: 'View redacted message history',
                      subtitle:
                          'Показывать историю у редактированных сообщений',
                      value: value,
                      onChanged: MayakSettings.setViewRedacted,
                    ),
                  ),
                ],
                ValueListenableBuilder<bool>(
                  valueListenable: MayakSettings.fullTimestamp,
                  builder: (context, value, _) => SettingsToggleTile(
                    icon: Symbols.schedule,
                    label: 'Точное время сообщений',
                    subtitle: 'Показывать время в секундах у сообщений',
                    value: value,
                    onChanged: MayakSettings.setFullTimestamp,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 20),
            const SectionHeader(
              'Папки',
              padding: EdgeInsets.fromLTRB(8, 0, 8, 8),
              fontSize: 14,
            ),
            SettingsCard(
              children: [
                ValueListenableBuilder<bool>(
                  valueListenable: MayakSettings.hideAllChatsFolder,
                  builder: (context, value, _) => SettingsToggleTile(
                    icon: Symbols.folder_off,
                    label: 'Скрыть папку «Все»',
                    subtitle:
                        'Скрыть папку «Все», когда есть другие папки. '
                        'Чаты сортируются только по вашим папкам',
                    value: value,
                    onChanged: MayakSettings.setHideAllChatsFolder,
                  ),
                ),
                ValueListenableBuilder<bool>(
                  valueListenable: MayakSettings.showHiddenChats,
                  builder: (context, value, _) => SettingsToggleTile(
                    icon: Symbols.visibility_lock,
                    label: 'Показывать скрытые чаты',
                    subtitle:
                        'Показывать скрытые чаты (например, от групповых '
                        'звонков), которые обычно не отображаются в списке',
                    value: value,
                    onChanged: MayakSettings.setShowHiddenChats,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 20),
            const SectionHeader(
              'Режим невидимки',
              padding: EdgeInsets.fromLTRB(8, 0, 8, 8),
              fontSize: 14,
            ),
            SettingsCard(
              children: [
                ValueListenableBuilder<bool>(
                  valueListenable: MayakSettings.ghostMode,
                  builder: (context, value, _) => SettingsToggleTile(
                    icon: Symbols.visibility_off,
                    label: 'Режим невидимки',
                    subtitle: 'Вас не видно в сети',
                    value: value,
                    onChanged: _setGhostMode,
                  ),
                ),
                ValueListenableBuilder<bool>(
                  valueListenable: MayakSettings.antiRead,
                  builder: (context, value, _) => SettingsToggleTile(
                    icon: Symbols.mark_chat_read,
                    label: 'Нечиталка',
                    subtitle: 'Нечиталка сообщений',
                    value: value,
                    onChanged: MayakSettings.setAntiRead,
                  ),
                ),
                ValueListenableBuilder<bool>(
                  valueListenable: MayakSettings.selfOnlineCheck,
                  builder: (context, value, _) => SettingsToggleTile(
                    icon: Symbols.radar,
                    label: 'Проверка своего онлайна',
                    subtitle:
                        'Каждые ~10 секунд сверяет, когда вы были онлайн. '
                        'Полезно для проверки ghost mode',
                    value: value,
                    onChanged: MayakSettings.setSelfOnlineCheck,
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _setGhostMode(bool value) async {
    await MayakSettings.setGhostMode(value);
    api.sendPing(interactive: !value);
  }
}
