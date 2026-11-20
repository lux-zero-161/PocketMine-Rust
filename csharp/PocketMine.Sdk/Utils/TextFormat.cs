namespace PocketMine.Utils;

/// <summary>Minecraft <c>§</c> colour codes, mirroring <c>pocketmine\utils\TextFormat</c>.</summary>
public static class TextFormat
{
    public const string Black = "§0";
    public const string DarkBlue = "§1";
    public const string DarkGreen = "§2";
    public const string DarkAqua = "§3";
    public const string DarkRed = "§4";
    public const string DarkPurple = "§5";
    public const string Gold = "§6";
    public const string Gray = "§7";
    public const string DarkGray = "§8";
    public const string Blue = "§9";
    public const string Green = "§a";
    public const string Aqua = "§b";
    public const string Red = "§c";
    public const string LightPurple = "§d";
    public const string Yellow = "§e";
    public const string White = "§f";

    public const string Obfuscated = "§k";
    public const string Bold = "§l";
    public const string Strikethrough = "§m";
    public const string Underline = "§n";
    public const string Italic = "§o";
    public const string Reset = "§r";

    public static string StripColors(string input)
    {
        var builder = new System.Text.StringBuilder(input.Length);
        for (int i = 0; i < input.Length; i++)
        {
            if (input[i] == '§' && i + 1 < input.Length)
            {
                i++;
                continue;
            }
            builder.Append(input[i]);
        }
        return builder.ToString();
    }
}
