using System.Text.Json;
using PocketMine.Event;

namespace PocketMine.Host;

/// <summary>
/// Serialises only the plugin-mutable fields of an event. The Rust core merges
/// this patch into its own event value, so e.g. changing
/// <c>PlayerChatEvent.Message</c> or <c>JoinMessage</c> takes effect on the
/// server without re-sending the whole payload.
/// </summary>
public static class EventPatcher
{
    public static byte[]? Patch(int eventType, Event.Event ev)
    {
        object? patch = eventType switch
        {
            1 when ev is PlayerJoinEvent e => new { join_message = e.JoinMessage },
            2 when ev is PlayerQuitEvent e => new { quit_message = e.QuitMessage },
            3 when ev is PlayerChatEvent e => new { message = e.Message, format = e.Format },
            6 when ev is ServerCommandEvent e => new { command = e.Command },
            7 when ev is PlayerCommandPreprocessEvent e => new { message = e.Message },
            8 when ev is PlayerMoveEvent e => new { to_x = e.To.X, to_y = e.To.Y, to_z = e.To.Z },
            10 when ev is EntityDamageEvent e => new { damage = e.Damage },
            11 when ev is PlayerDeathEvent e => new { death_message = e.DeathMessage },
            12 when ev is PlayerRespawnEvent e => new { x = e.Position.X, y = e.Position.Y, z = e.Position.Z },
            _ => null,
        };
        return patch == null ? null : JsonSerializer.SerializeToUtf8Bytes(patch);
    }
}
