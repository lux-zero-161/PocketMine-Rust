using PocketMine.Permission;
using PocketMine.Player;

namespace PocketMine.Host;

/// <summary>
/// Permission defaults registered by plugins. Node overrides for individual
/// players would be stored as attachments; the server core already owns the
/// operator flag, which is the part plugins rely on most.
/// </summary>
public sealed class HostPermissionRegistry
{
    private readonly Dictionary<string, (string Description, PermissionDefault Default)> _permissions =
        new(StringComparer.OrdinalIgnoreCase);

    public void Register(string name, string description, PermissionDefault @default)
        => _permissions[name] = (description, @default);

    public bool IsRegistered(string name) => _permissions.ContainsKey(name);

    public bool HasPermission(Player.Player? player, string node)
    {
        if (player?.IsOp == true)
        {
            return true;
        }
        if (_permissions.TryGetValue(node, out var entry))
        {
            return entry.Default switch
            {
                PermissionDefault.True => true,
                PermissionDefault.Op => player?.IsOp == true,
                PermissionDefault.NotOp => player?.IsOp != true,
                _ => false,
            };
        }
        // Unknown nodes are denied for regular players, like PMMP.
        return false;
    }

    public IReadOnlyDictionary<string, (string Description, PermissionDefault Default)> All => _permissions;
}
