//! Minecraft `§` text formatting helpers, equivalent to `pocketmine\utils\TextFormat`.

pub struct TextFormat;

#[allow(dead_code)]
impl TextFormat {
    pub const ESCAPE: char = '§';

    pub const BLACK: &'static str = "§0";
    pub const DARK_BLUE: &'static str = "§1";
    pub const DARK_GREEN: &'static str = "§2";
    pub const DARK_AQUA: &'static str = "§3";
    pub const DARK_RED: &'static str = "§4";
    pub const DARK_PURPLE: &'static str = "§5";
    pub const GOLD: &'static str = "§6";
    pub const GRAY: &'static str = "§7";
    pub const DARK_GRAY: &'static str = "§8";
    pub const BLUE: &'static str = "§9";
    pub const GREEN: &'static str = "§a";
    pub const AQUA: &'static str = "§b";
    pub const RED: &'static str = "§c";
    pub const LIGHT_PURPLE: &'static str = "§d";
    pub const YELLOW: &'static str = "§e";
    pub const WHITE: &'static str = "§f";
    pub const MINECOIN_GOLD: &'static str = "§g";

    pub const OBFUSCATED: &'static str = "§k";
    pub const BOLD: &'static str = "§l";
    pub const STRIKETHROUGH: &'static str = "§m";
    pub const UNDERLINE: &'static str = "§n";
    pub const ITALIC: &'static str = "§o";
    pub const RESET: &'static str = "§r";
}

/// Removes every `§x` formatting sequence.
pub fn strip_colors(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
            continue;
        }
        out.push(c);
    }
    out
}

/// Translates `§` colour codes into ANSI escape sequences for the terminal.
pub fn to_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '§' {
            out.push(c);
            continue;
        }
        let Some(code) = chars.next() else { break };
        out.push_str(match code.to_ascii_lowercase() {
            '0' => "\x1b[30m",
            '1' => "\x1b[34m",
            '2' => "\x1b[32m",
            '3' => "\x1b[36m",
            '4' => "\x1b[31m",
            '5' => "\x1b[35m",
            '6' => "\x1b[33m",
            '7' => "\x1b[37m",
            '8' => "\x1b[90m",
            '9' => "\x1b[94m",
            'a' => "\x1b[92m",
            'b' => "\x1b[96m",
            'c' => "\x1b[91m",
            'd' => "\x1b[95m",
            'e' => "\x1b[93m",
            'f' => "\x1b[97m",
            'g' => "\x1b[93m",
            'k' => "\x1b[8m",
            'l' => "\x1b[1m",
            'm' => "\x1b[9m",
            'n' => "\x1b[4m",
            'o' => "\x1b[3m",
            'r' => "\x1b[0m",
            other => {
                // Not a known code: keep it visible instead of swallowing text.
                out.push('§');
                out.push(other);
                continue;
            }
        });
    }
    out.push_str("\x1b[0m");
    out
}

/// Returns true when the string contains formatting sequences.
pub fn has_colors(input: &str) -> bool {
    input.contains('§')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_codes() {
        assert_eq!(strip_colors("§aHello §lWorld§r!"), "Hello World!");
        assert_eq!(strip_colors("no codes"), "no codes");
    }

    #[test]
    fn ansi_wraps() {
        let s = to_ansi("§cRed");
        assert!(s.starts_with("\x1b[91m"));
        assert!(s.ends_with("\x1b[0m"));
    }
}
