using PocketMine.Utils;

namespace PocketMine.Server;

/// <summary>Snapshot of the server, safe to cache inside plugins.</summary>
public sealed record ServerInfo
{
    public string Name { get; init; } = "";
    public string Version { get; init; } = "";
    public string ApiVersion { get; init; } = "";
    public string MinecraftVersion { get; init; } = "";
    public int Protocol { get; init; }
    public ulong Tick { get; init; }
    public double Tps { get; init; } = 20;
    public int Online { get; init; }
    public int MaxPlayers { get; init; }
    public long Uptime { get; init; }
    public string Motd { get; init; } = "";
    public IReadOnlyList<string> Worlds { get; init; } = Array.Empty<string>();
    public IReadOnlyList<string> Plugins { get; init; } = Array.Empty<string>();

    public override string ToString() =>
        $"{Name} {Version} (MC {MinecraftVersion}, protocol {Protocol})";
}
