using PocketMine.Event;
using PocketMine.Plugin;

namespace PocketMine.Server;

public interface IServer
{
    void BroadcastMessage(string message);
    void RegisterCommand(PocketMine.Command.Command command);
    void RegisterEvents(IListener listener, PluginBase plugin);
    void Shutdown();
}
