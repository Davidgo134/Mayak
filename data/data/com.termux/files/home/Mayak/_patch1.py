import io

path = "lib/frontend/screens/chats/chat_screen.dart"
with io.open(path, encoding="utf-8") as f:
    content = f.read()

old_import = "import '../../widgets/message_bubble.dart';\n"
new_import = (
    "import '../../widgets/message_bubble.dart';\n"
    "import '../../widgets/attachment/bubbles/video_note_bubble.dart';\n"
)
assert content.count(old_import) == 1, "import anchor not found or not unique"
content = content.replace(old_import, new_import, 1)

old_body = (
    "                builder: (context, body) => Scaffold(\n"
    "                  backgroundColor: cs.surface,\n"
    "                  extendBodyBehindAppBar: underlap,\n"
    "                  appBar: _buildAppBar(cs),\n"
    "                  body: body,\n"
    "                ),\n"
)
new_body = (
    "                builder: (context, body) => Scaffold(\n"
    "                  backgroundColor: cs.surface,\n"
    "                  extendBodyBehindAppBar: underlap,\n"
    "                  appBar: _buildAppBar(cs),\n"
    "                  body: Listener(\n"
    "                    behavior: HitTestBehavior.translucent,\n"
    "                    onPointerDown: (event) =>\n"
    "                        VideoNoteBubble.collapseOutside(event.position),\n"
    "                    child: body,\n"
    "                  ),\n"
    "                ),\n"
)
assert content.count(old_body) == 1, "body anchor not found or not unique"
content = content.replace(old_body, new_body, 1)

with io.open(path, "w", encoding="utf-8") as f:
    f.write(content)

print("OK")
