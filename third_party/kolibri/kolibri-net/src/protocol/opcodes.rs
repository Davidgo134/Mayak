//! Protocol operation codes, from `lib/core/protocol/opcode_map.dart`.

// ── Session ──────────────────────────────────────────────────────────────
pub const PING: u16 = 1;
pub const DEBUG: u16 = 2;
pub const RECONNECT: u16 = 3;
pub const LOG: u16 = 5;
pub const SESSION_INIT: u16 = 6;
pub const CONTACTS_GET: u16 = 8;

// ── Profile ──────────────────────────────────────────────────────────────
pub const PROFILE: u16 = 16;

// ── Auth ─────────────────────────────────────────────────────────────────
pub const AUTH_REQUEST: u16 = 17;
pub const AUTH: u16 = 18;
pub const LOGIN: u16 = 19;
pub const LOGOUT: u16 = 20;
pub const SYNC: u16 = 21;
pub const CONFIG: u16 = 22;
pub const AUTH_CONFIRM: u16 = 23;

// ── Auth 2FA ─────────────────────────────────────────────────────────────
pub const AUTH_LOGIN_RESTORE_PASSWORD: u16 = 101;
pub const AUTH_2FA_DETAILS: u16 = 104;
pub const EXTERNAL_CALLBACK: u16 = 105;
pub const AUTH_VALIDATE_PASSWORD: u16 = 107;
pub const AUTH_VALIDATE_HINT: u16 = 108;
pub const AUTH_VERIFY_EMAIL: u16 = 109;
pub const AUTH_CHECK_EMAIL: u16 = 110;
pub const AUTH_SET_2FA: u16 = 111;
pub const AUTH_CREATE_TRACK: u16 = 112;
pub const AUTH_CHECK_PASSWORD: u16 = 113;
pub const AUTH_LOGIN_CHECK_PASSWORD: u16 = 115;
pub const AUTH_LOGIN_PROFILE_DELETE: u16 = 116;

// ── Assets ───────────────────────────────────────────────────────────────
pub const PRESET_AVATARS: u16 = 25;
pub const ASSETS_GET: u16 = 26;
pub const ASSETS_UPDATE: u16 = 27;
pub const ASSETS_GET_BY_IDS: u16 = 28;
pub const ASSETS_ADD: u16 = 29;
pub const ASSETS_REMOVE: u16 = 259;
pub const ASSETS_MOVE: u16 = 260;
pub const ASSETS_LIST_MODIFY: u16 = 261;

// ── Contacts ─────────────────────────────────────────────────────────────
pub const CONTACT_INFO: u16 = 32;
pub const CONTACT_ADD: u16 = 33;
pub const CONTACT_UPDATE: u16 = 34;
pub const CONTACT_PRESENCE: u16 = 35;
pub const CONTACT_LIST: u16 = 36;
pub const CONTACT_SEARCH: u16 = 37;
pub const CONTACT_MUTUAL: u16 = 38;
pub const CONTACT_PHOTOS: u16 = 39;
pub const CONTACT_SORT: u16 = 40;
pub const CONTACT_VERIFY: u16 = 42;
pub const REMOVE_CONTACT_PHOTO: u16 = 43;
pub const CONTACT_INFO_BY_PHONE: u16 = 46;

// ── Chats ────────────────────────────────────────────────────────────────
pub const CHAT_INFO: u16 = 48;
pub const CHAT_HISTORY: u16 = 49;
pub const CHAT_MARK: u16 = 50;
pub const CHAT_MEDIA: u16 = 51;
pub const CHAT_DELETE: u16 = 52;
pub const CHATS_LIST: u16 = 53;
pub const CHAT_CLEAR: u16 = 54;
pub const CHAT_UPDATE: u16 = 55;
pub const CHAT_CHECK_LINK: u16 = 56;
pub const CHAT_JOIN: u16 = 57;
pub const CHAT_LEAVE: u16 = 58;
pub const CHAT_MEMBERS: u16 = 59;
pub const PUBLIC_SEARCH: u16 = 60;
pub const CHAT_PERSONAL_CONFIG: u16 = 61;
pub const CHAT_CREATE: u16 = 63;

// ── Messages ─────────────────────────────────────────────────────────────
pub const MSG_SEND: u16 = 64;
pub const MSG_TYPING: u16 = 65;
pub const MSG_DELETE: u16 = 66;
pub const MSG_EDIT: u16 = 67;
pub const CHAT_SEARCH: u16 = 68;
pub const MSG_SHARE_PREVIEW: u16 = 70;
pub const MSG_GET: u16 = 71;
pub const MSG_SEARCH_TOUCH: u16 = 72;
pub const MSG_SEARCH: u16 = 73;
pub const MSG_GET_STAT: u16 = 74;
pub const CHAT_SUBSCRIBE: u16 = 75;
pub const MSG_DELETE_RANGE: u16 = 92;

// ── Reactions ────────────────────────────────────────────────────────────
pub const MSG_REACTION: u16 = 178;
pub const MSG_CANCEL_REACTION: u16 = 179;
pub const MSG_GET_REACTIONS: u16 = 180;
pub const MSG_GET_DETAILED_REACTIONS: u16 = 181;
pub const CHAT_REACTIONS_SETTINGS_SET: u16 = 257;
pub const REACTIONS_SETTINGS_GET_BY_CHAT_ID: u16 = 258;

// ── Calls & Video ────────────────────────────────────────────────────────
pub const VIDEO_CHAT_START: u16 = 76;
pub const CHAT_MEMBERS_UPDATE: u16 = 77;
pub const VIDEO_CHAT_START_ACTIVE: u16 = 78;
pub const VIDEO_CHAT_HISTORY: u16 = 79;
pub const VIDEO_CHAT_DELETE_HISTORY: u16 = 164;
pub const VIDEO_CHAT_CREATE_JOIN_LINK: u16 = 84;
pub const VIDEO_CHAT_JOIN_BY_LINK: u16 = 166;
pub const VIDEO_CHAT_MEMBERS: u16 = 195;
pub const GET_INBOUND_CALLS: u16 = 103;

// ── Media & Files ────────────────────────────────────────────────────────
pub const PHOTO_UPLOAD: u16 = 80;
pub const STICKER_UPLOAD: u16 = 81;
pub const VIDEO_UPLOAD: u16 = 82;
pub const VIDEO_PLAY: u16 = 83;
pub const CHAT_PIN_SET_VISIBILITY: u16 = 86;
pub const FILE_UPLOAD: u16 = 87;
pub const FILE_DOWNLOAD: u16 = 88;
pub const LINK_INFO: u16 = 89;
pub const AUDIO_PLAY: u16 = 301;

// ── Sessions ─────────────────────────────────────────────────────────────
pub const SESSIONS_INFO: u16 = 96;
pub const SESSIONS_CLOSE: u16 = 97;
pub const PHONE_BIND_REQUEST: u16 = 98;
pub const PHONE_BIND_CONFIRM: u16 = 99;

// ── Bots ─────────────────────────────────────────────────────────────────
pub const CHAT_COMPLAIN: u16 = 117;
pub const MSG_SEND_CALLBACK: u16 = 118;
pub const SUSPEND_BOT: u16 = 119;
pub const CHAT_BOT_COMMANDS: u16 = 144;
pub const BOT_INFO: u16 = 145;

// ── Location ─────────────────────────────────────────────────────────────
pub const LOCATION_STOP: u16 = 124;

// ── Mentions ─────────────────────────────────────────────────────────────
pub const GET_LAST_MENTIONS: u16 = 127;

// ── Stickers (creation) ──────────────────────────────────────────────────
pub const STICKER_CREATE: u16 = 193;
pub const STICKER_SUGGEST: u16 = 194;

// ── Notifications (server push) ──────────────────────────────────────────
pub const NOTIF_MESSAGE: u16 = 128;
pub const NOTIF_TYPING: u16 = 129;
pub const NOTIF_MARK: u16 = 130;
pub const NOTIF_CONTACT: u16 = 131;
pub const NOTIF_PRESENCE: u16 = 132;
pub const NOTIF_CONFIG: u16 = 134;
pub const NOTIF_CHAT: u16 = 135;
pub const NOTIF_ATTACH: u16 = 136;
pub const NOTIF_CALL_START: u16 = 137;
pub const NOTIF_CONTACT_SORT: u16 = 139;
pub const NOTIF_MSG_DELETE_RANGE: u16 = 140;
pub const NOTIF_MSG_DELETE: u16 = 142;
pub const NOTIF_CALLBACK_ANSWER: u16 = 143;
pub const NOTIF_LOCATION: u16 = 147;
pub const NOTIF_LOCATION_REQUEST: u16 = 148;
pub const NOTIF_ASSETS_UPDATE: u16 = 150;
pub const NOTIF_DRAFT: u16 = 152;
pub const NOTIF_DRAFT_DISCARD: u16 = 153;
pub const NOTIF_MSG_DELAYED: u16 = 154;
pub const NOTIF_MSG_REACTIONS_CHANGED: u16 = 155;
pub const NOTIF_MSG_YOU_REACTED: u16 = 156;
pub const NOTIF_PROFILE: u16 = 159;
pub const NOTIF_BANNERS: u16 = 292;
pub const NOTIF_FOLDERS: u16 = 277;

// ── Transcription ────────────────────────────────────────────────────────
pub const AUDIO_TRANSCRIPTION: u16 = 202;
pub const TRANSCRIPTION_RESULT: u16 = 293;

// ── Misc ─────────────────────────────────────────────────────────────────
pub const OK_TOKEN: u16 = 158;
pub const WEB_APP_INIT_DATA: u16 = 160;
pub const COMPLAIN: u16 = 161;
pub const COMPLAIN_REASONS_GET: u16 = 162;
pub const DRAFT_SAVE: u16 = 176;
pub const DRAFT_DISCARD: u16 = 177;
pub const CHAT_HIDE: u16 = 196;
pub const CHAT_SEARCH_COMMON_PARTICIPANTS: u16 = 198;
pub const PROFILE_DELETE: u16 = 199;
pub const PROFILE_DELETE_TIME: u16 = 200;
pub const AUTH_QR_APPROVE: u16 = 290;
pub const CHAT_SUGGEST: u16 = 300;

// ── Polls ────────────────────────────────────────────────────────────────
pub const SEND_VOTE: u16 = 304;
pub const VOTERS_LIST_BY_ANSWER: u16 = 305;
pub const GET_POLL_UPDATES: u16 = 306;

// ── Folders ──────────────────────────────────────────────────────────────
pub const FOLDERS_GET: u16 = 272;
pub const FOLDERS_GET_BY_ID: u16 = 273;
pub const FOLDERS_UPDATE: u16 = 274;
pub const FOLDERS_REORDER: u16 = 275;
pub const FOLDERS_DELETE: u16 = 276;

// ── Stories ──────────────────────────────────────────────────────────────
pub const STORIES_LIST: u16 = 208;
pub const STORIES_LIST_BY_OWNER: u16 = 209;
pub const STORIES_GET_BY_OWNER: u16 = 210;
pub const STORIES_GET_STATS: u16 = 211;
pub const STORIES_GET_DETAILED_STATS: u16 = 212;
pub const STORIES_REACT: u16 = 213;
pub const STORIES_MARK: u16 = 214;
pub const STORIES_SEND: u16 = 215;
pub const NOTIF_STORIES_UPDATE: u16 = 216;
pub const STORIES_EDIT: u16 = 217;
pub const STORIES_DELETE: u16 = 218;
pub const STORIES_GET_BY_STORY_ID: u16 = 220;

/// Label for an opcode, or `UNKNOWN(n)` if unmapped.
pub fn name(opcode: u16) -> String {
    match opcode {
        PING => "PING".into(),
        DEBUG => "DEBUG".into(),
        RECONNECT => "RECONNECT".into(),
        LOG => "LOG".into(),
        SESSION_INIT => "SESSION_INIT".into(),
        CONTACTS_GET => "CONTACTS_GET".into(),
        PROFILE => "PROFILE".into(),
        AUTH_REQUEST => "AUTH_REQUEST".into(),
        AUTH => "AUTH".into(),
        LOGIN => "LOGIN".into(),
        LOGOUT => "LOGOUT".into(),
        SYNC => "SYNC".into(),
        CONFIG => "CONFIG".into(),
        AUTH_CONFIRM => "AUTH_CONFIRM".into(),
        AUTH_LOGIN_RESTORE_PASSWORD => "AUTH_LOGIN_RESTORE_PASSWORD".into(),
        AUTH_2FA_DETAILS => "AUTH_2FA_DETAILS".into(),
        EXTERNAL_CALLBACK => "EXTERNAL_CALLBACK".into(),
        AUTH_VALIDATE_PASSWORD => "AUTH_VALIDATE_PASSWORD".into(),
        AUTH_VALIDATE_HINT => "AUTH_VALIDATE_HINT".into(),
        AUTH_VERIFY_EMAIL => "AUTH_VERIFY_EMAIL".into(),
        AUTH_CHECK_EMAIL => "AUTH_CHECK_EMAIL".into(),
        AUTH_SET_2FA => "AUTH_SET_2FA".into(),
        AUTH_CREATE_TRACK => "AUTH_CREATE_TRACK".into(),
        AUTH_CHECK_PASSWORD => "AUTH_CHECK_PASSWORD".into(),
        AUTH_LOGIN_CHECK_PASSWORD => "AUTH_LOGIN_CHECK_PASSWORD".into(),
        AUTH_LOGIN_PROFILE_DELETE => "AUTH_LOGIN_PROFILE_DELETE".into(),
        PRESET_AVATARS => "PRESET_AVATARS".into(),
        ASSETS_GET => "ASSETS_GET".into(),
        ASSETS_UPDATE => "ASSETS_UPDATE".into(),
        ASSETS_GET_BY_IDS => "ASSETS_GET_BY_IDS".into(),
        ASSETS_ADD => "ASSETS_ADD".into(),
        ASSETS_REMOVE => "ASSETS_REMOVE".into(),
        ASSETS_MOVE => "ASSETS_MOVE".into(),
        ASSETS_LIST_MODIFY => "ASSETS_LIST_MODIFY".into(),
        CONTACT_INFO => "CONTACT_INFO".into(),
        CONTACT_ADD => "CONTACT_ADD".into(),
        CONTACT_UPDATE => "CONTACT_UPDATE".into(),
        CONTACT_PRESENCE => "CONTACT_PRESENCE".into(),
        CONTACT_LIST => "CONTACT_LIST".into(),
        CONTACT_SEARCH => "CONTACT_SEARCH".into(),
        CONTACT_MUTUAL => "CONTACT_MUTUAL".into(),
        CONTACT_PHOTOS => "CONTACT_PHOTOS".into(),
        CONTACT_SORT => "CONTACT_SORT".into(),
        CONTACT_VERIFY => "CONTACT_VERIFY".into(),
        REMOVE_CONTACT_PHOTO => "REMOVE_CONTACT_PHOTO".into(),
        CONTACT_INFO_BY_PHONE => "CONTACT_INFO_BY_PHONE".into(),
        CHAT_INFO => "CHAT_INFO".into(),
        CHAT_HISTORY => "CHAT_HISTORY".into(),
        CHAT_MARK => "CHAT_MARK".into(),
        CHAT_MEDIA => "CHAT_MEDIA".into(),
        CHAT_DELETE => "CHAT_DELETE".into(),
        CHATS_LIST => "CHATS_LIST".into(),
        CHAT_CLEAR => "CHAT_CLEAR".into(),
        CHAT_UPDATE => "CHAT_UPDATE".into(),
        CHAT_CHECK_LINK => "CHAT_CHECK_LINK".into(),
        CHAT_JOIN => "CHAT_JOIN".into(),
        CHAT_LEAVE => "CHAT_LEAVE".into(),
        CHAT_MEMBERS => "CHAT_MEMBERS".into(),
        PUBLIC_SEARCH => "PUBLIC_SEARCH".into(),
        CHAT_PERSONAL_CONFIG => "CHAT_PERSONAL_CONFIG".into(),
        CHAT_CREATE => "CHAT_CREATE".into(),
        MSG_SEND => "MSG_SEND".into(),
        MSG_TYPING => "MSG_TYPING".into(),
        MSG_DELETE => "MSG_DELETE".into(),
        MSG_EDIT => "MSG_EDIT".into(),
        CHAT_SEARCH => "CHAT_SEARCH".into(),
        MSG_SHARE_PREVIEW => "MSG_SHARE_PREVIEW".into(),
        MSG_GET => "MSG_GET".into(),
        MSG_SEARCH_TOUCH => "MSG_SEARCH_TOUCH".into(),
        MSG_SEARCH => "MSG_SEARCH".into(),
        MSG_GET_STAT => "MSG_GET_STAT".into(),
        CHAT_SUBSCRIBE => "CHAT_SUBSCRIBE".into(),
        MSG_DELETE_RANGE => "MSG_DELETE_RANGE".into(),
        MSG_REACTION => "MSG_REACTION".into(),
        MSG_CANCEL_REACTION => "MSG_CANCEL_REACTION".into(),
        MSG_GET_REACTIONS => "MSG_GET_REACTIONS".into(),
        MSG_GET_DETAILED_REACTIONS => "MSG_GET_DETAILED_REACTIONS".into(),
        CHAT_REACTIONS_SETTINGS_SET => "CHAT_REACTIONS_SETTINGS_SET".into(),
        REACTIONS_SETTINGS_GET_BY_CHAT_ID => "REACTIONS_SETTINGS_GET_BY_CHAT_ID".into(),
        VIDEO_CHAT_START => "VIDEO_CHAT_START".into(),
        CHAT_MEMBERS_UPDATE => "CHAT_MEMBERS_UPDATE".into(),
        VIDEO_CHAT_START_ACTIVE => "VIDEO_CHAT_START_ACTIVE".into(),
        VIDEO_CHAT_HISTORY => "VIDEO_CHAT_HISTORY".into(),
        VIDEO_CHAT_DELETE_HISTORY => "VIDEO_CHAT_DELETE_HISTORY".into(),
        VIDEO_CHAT_CREATE_JOIN_LINK => "VIDEO_CHAT_CREATE_JOIN_LINK".into(),
        VIDEO_CHAT_JOIN_BY_LINK => "VIDEO_CHAT_JOIN_BY_LINK".into(),
        VIDEO_CHAT_MEMBERS => "VIDEO_CHAT_MEMBERS".into(),
        GET_INBOUND_CALLS => "GET_INBOUND_CALLS".into(),
        PHOTO_UPLOAD => "PHOTO_UPLOAD".into(),
        STICKER_UPLOAD => "STICKER_UPLOAD".into(),
        VIDEO_UPLOAD => "VIDEO_UPLOAD".into(),
        VIDEO_PLAY => "VIDEO_PLAY".into(),
        CHAT_PIN_SET_VISIBILITY => "CHAT_PIN_SET_VISIBILITY".into(),
        FILE_UPLOAD => "FILE_UPLOAD".into(),
        FILE_DOWNLOAD => "FILE_DOWNLOAD".into(),
        LINK_INFO => "LINK_INFO".into(),
        AUDIO_PLAY => "AUDIO_PLAY".into(),
        SESSIONS_INFO => "SESSIONS_INFO".into(),
        SESSIONS_CLOSE => "SESSIONS_CLOSE".into(),
        PHONE_BIND_REQUEST => "PHONE_BIND_REQUEST".into(),
        PHONE_BIND_CONFIRM => "PHONE_BIND_CONFIRM".into(),
        CHAT_COMPLAIN => "CHAT_COMPLAIN".into(),
        MSG_SEND_CALLBACK => "MSG_SEND_CALLBACK".into(),
        SUSPEND_BOT => "SUSPEND_BOT".into(),
        CHAT_BOT_COMMANDS => "CHAT_BOT_COMMANDS".into(),
        BOT_INFO => "BOT_INFO".into(),
        LOCATION_STOP => "LOCATION_STOP".into(),
        GET_LAST_MENTIONS => "GET_LAST_MENTIONS".into(),
        STICKER_CREATE => "STICKER_CREATE".into(),
        STICKER_SUGGEST => "STICKER_SUGGEST".into(),
        NOTIF_MESSAGE => "NOTIF_MESSAGE".into(),
        NOTIF_TYPING => "NOTIF_TYPING".into(),
        NOTIF_MARK => "NOTIF_MARK".into(),
        NOTIF_CONTACT => "NOTIF_CONTACT".into(),
        NOTIF_PRESENCE => "NOTIF_PRESENCE".into(),
        NOTIF_CONFIG => "NOTIF_CONFIG".into(),
        NOTIF_CHAT => "NOTIF_CHAT".into(),
        NOTIF_ATTACH => "NOTIF_ATTACH".into(),
        NOTIF_CALL_START => "NOTIF_CALL_START".into(),
        NOTIF_CONTACT_SORT => "NOTIF_CONTACT_SORT".into(),
        NOTIF_MSG_DELETE_RANGE => "NOTIF_MSG_DELETE_RANGE".into(),
        NOTIF_MSG_DELETE => "NOTIF_MSG_DELETE".into(),
        NOTIF_CALLBACK_ANSWER => "NOTIF_CALLBACK_ANSWER".into(),
        NOTIF_LOCATION => "NOTIF_LOCATION".into(),
        NOTIF_LOCATION_REQUEST => "NOTIF_LOCATION_REQUEST".into(),
        NOTIF_ASSETS_UPDATE => "NOTIF_ASSETS_UPDATE".into(),
        NOTIF_DRAFT => "NOTIF_DRAFT".into(),
        NOTIF_DRAFT_DISCARD => "NOTIF_DRAFT_DISCARD".into(),
        NOTIF_MSG_DELAYED => "NOTIF_MSG_DELAYED".into(),
        NOTIF_MSG_REACTIONS_CHANGED => "NOTIF_MSG_REACTIONS_CHANGED".into(),
        NOTIF_MSG_YOU_REACTED => "NOTIF_MSG_YOU_REACTED".into(),
        NOTIF_PROFILE => "NOTIF_PROFILE".into(),
        NOTIF_BANNERS => "NOTIF_BANNERS".into(),
        NOTIF_FOLDERS => "NOTIF_FOLDERS".into(),
        AUDIO_TRANSCRIPTION => "AUDIO_TRANSCRIPTION".into(),
        TRANSCRIPTION_RESULT => "TRANSCRIPTION_RESULT".into(),
        OK_TOKEN => "OK_TOKEN".into(),
        WEB_APP_INIT_DATA => "WEB_APP_INIT_DATA".into(),
        COMPLAIN => "COMPLAIN".into(),
        COMPLAIN_REASONS_GET => "COMPLAIN_REASONS_GET".into(),
        DRAFT_SAVE => "DRAFT_SAVE".into(),
        DRAFT_DISCARD => "DRAFT_DISCARD".into(),
        CHAT_HIDE => "CHAT_HIDE".into(),
        CHAT_SEARCH_COMMON_PARTICIPANTS => "CHAT_SEARCH_COMMON_PARTICIPANTS".into(),
        PROFILE_DELETE => "PROFILE_DELETE".into(),
        PROFILE_DELETE_TIME => "PROFILE_DELETE_TIME".into(),
        AUTH_QR_APPROVE => "AUTH_QR_APPROVE".into(),
        CHAT_SUGGEST => "CHAT_SUGGEST".into(),
        SEND_VOTE => "SEND_VOTE".into(),
        VOTERS_LIST_BY_ANSWER => "VOTERS_LIST_BY_ANSWER".into(),
        GET_POLL_UPDATES => "GET_POLL_UPDATES".into(),
        FOLDERS_GET => "FOLDERS_GET".into(),
        FOLDERS_GET_BY_ID => "FOLDERS_GET_BY_ID".into(),
        FOLDERS_UPDATE => "FOLDERS_UPDATE".into(),
        FOLDERS_REORDER => "FOLDERS_REORDER".into(),
        FOLDERS_DELETE => "FOLDERS_DELETE".into(),
        STORIES_LIST => "STORIES_LIST".into(),
        STORIES_LIST_BY_OWNER => "STORIES_LIST_BY_OWNER_ID".into(),
        STORIES_GET_BY_OWNER => "STORIES_GET_BY_OWNER_ID".into(),
        STORIES_GET_STATS => "STORIES_GET_STATS".into(),
        STORIES_GET_DETAILED_STATS => "STORIES_GET_DETAILED_STATS".into(),
        STORIES_REACT => "STORIES_REACT".into(),
        STORIES_MARK => "STORIES_MARK".into(),
        STORIES_SEND => "STORIES_SEND".into(),
        NOTIF_STORIES_UPDATE => "NOTIF_STORIES_UPDATE".into(),
        STORIES_EDIT => "STORIES_EDIT".into(),
        STORIES_DELETE => "STORIES_DELETE".into(),
        STORIES_GET_BY_STORY_ID => "STORIES_GET_BY_STORY_ID".into(),
        other => format!("UNKNOWN({other})"),
    }
}
