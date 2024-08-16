use std::convert::TryFrom;

/// IconId is a usize that represents an icon in the database
/// The value is the index of the icon in the database's icon list
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct IconId(pub usize);

impl std::fmt::Display for IconId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const EMOJIS: [&str; 69] = [
            "🔑", "🌍", "⚠️", "🖥️", "📁", "💬", "🔧", "📝", "🌐", "🆔", "📄", "📷", "📡", "🔑", "🔌", "📱", "🔖", "💿", "🖥️", "📧", "🔧",
            "📋", "🆕", "📸", "⚡", "📻", "💾", "🌐", "🎞️", "🔒", "💻", "🖨️", "🔳", "🏁", "🔧", "🌐", "🗜️", "💯", "💻", "🕰️", "🔍", "🏞️",
            "💾", "🗑️", "📋", "🛑", "ℹ️", "🗄️", "📁", "📂", "🖥️", "🔓", "🔒", "✅", "🖊️", "📷", "👥", "📊", "🔒", "🔧", "🏠", "⭐", "🐧",
            "🤖", "🍎", "🌐", "💵", "📜", "📱",
        ];

        let emoji = EMOJIS.get(self.0).unwrap_or(&"");
        write!(f, "{}", emoji)
    }
}

impl IconId {
    pub const KEY: IconId = IconId(0);
    pub const WORLD: IconId = IconId(1);
    pub const WARNING: IconId = IconId(2);
    pub const NETWORK_SERVER: IconId = IconId(3);
    pub const MARKED_DIRECTORY: IconId = IconId(4);
    pub const USER_COMMUNICATION: IconId = IconId(5);
    pub const PARTS: IconId = IconId(6);
    pub const NOTEPAD: IconId = IconId(7);
    pub const WORLD_SOCKET: IconId = IconId(8);
    pub const IDENTITY: IconId = IconId(9);
    pub const PAPER_READY: IconId = IconId(10);
    pub const DIGICAM: IconId = IconId(11);
    pub const IRCOMMUNICATION: IconId = IconId(12);
    pub const MULTI_KEYS: IconId = IconId(13);
    pub const PLUG: IconId = IconId(14);
    pub const PDA: IconId = IconId(15);
    pub const BOOK_MARK: IconId = IconId(16);
    pub const CD_ROM: IconId = IconId(17);
    pub const MONITOR: IconId = IconId(18);
    pub const EMAIL: IconId = IconId(19);
    pub const CONFIG: IconId = IconId(20);
    pub const CLIPBOARD_READY: IconId = IconId(21);
    pub const PAPER_NEW: IconId = IconId(22);
    pub const SCREENSHOT: IconId = IconId(23);
    pub const THUNDER: IconId = IconId(24);
    pub const RADIO: IconId = IconId(25);
    pub const FLOPPY_DISK: IconId = IconId(26);
    pub const FTP: IconId = IconId(27);
    pub const FILM: IconId = IconId(28);
    pub const SECURITY_TERMINAL: IconId = IconId(29);
    pub const TERMINAL: IconId = IconId(30);
    pub const PRINTER: IconId = IconId(31);
    pub const GRID: IconId = IconId(32);
    pub const CHECKER_BOARD: IconId = IconId(33);
    pub const WRENCH: IconId = IconId(34);
    pub const INTERNET: IconId = IconId(35);
    pub const ZIP_FOLDER: IconId = IconId(36);
    pub const PERCENT: IconId = IconId(37);
    pub const WINDOWS_PC: IconId = IconId(38);
    pub const CLOCK: IconId = IconId(39);
    pub const SEARCH: IconId = IconId(40);
    pub const LANDSCAPE: IconId = IconId(41);
    pub const MEMORY: IconId = IconId(42);
    pub const RECYCLE_BIN: IconId = IconId(43);
    pub const CLIPBOARD: IconId = IconId(44);
    pub const STOP: IconId = IconId(45);
    pub const INFORMATION: IconId = IconId(46);
    pub const FILING_CABINET: IconId = IconId(47);
    pub const FOLDER: IconId = IconId(48);
    pub const FOLDER_OPEN: IconId = IconId(49);
    pub const DESKTOP: IconId = IconId(50);
    pub const LOCK_OPEN: IconId = IconId(51);
    pub const LOCKED: IconId = IconId(52);
    pub const APPROVED: IconId = IconId(53);
    pub const MARKER: IconId = IconId(54);
    pub const PICTURE_DOC: IconId = IconId(55);
    pub const CONTACT: IconId = IconId(56);
    pub const EXCEL_SHEET: IconId = IconId(57);
    pub const SECURIT_ACCOUNT: IconId = IconId(58);
    pub const REPAIR: IconId = IconId(59);
    pub const HOME: IconId = IconId(60);
    pub const STAR: IconId = IconId(61);
    pub const LINUX: IconId = IconId(62);
    pub const ANDROID: IconId = IconId(63);
    pub const APPLE: IconId = IconId(64);
    pub const WIKIPEDIA: IconId = IconId(65);
    pub const DOLLAR: IconId = IconId(66);
    pub const CERTIFICATE: IconId = IconId(67);
    pub const MOBILE_PHONE: IconId = IconId(68);
}

impl TryFrom<usize> for IconId {
    type Error = crate::error::Error;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value > 68 {
            return Err(crate::error::Error::ParseIconIdError { icon_id: value });
        }
        Ok(Self(value))
    }
}

impl From<IconId> for usize {
    fn from(icon_id: IconId) -> Self {
        icon_id.0
    }
}
