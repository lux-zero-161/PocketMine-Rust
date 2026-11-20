namespace PocketMine.Scheduler;

public sealed class TaskHandler
{
    public int Id { get; internal set; }
    public bool IsCancelled { get; internal set; }
    public bool IsRepeating { get; internal set; }
    public string PluginName { get; internal set; } = string.Empty;

    internal Action? CancelAction;

    public void Cancel()
    {
        if (IsCancelled) return;
        IsCancelled = true;
        CancelAction?.Invoke();
    }

    public override string ToString() => $"TaskHandler(#{Id}, {PluginName}, cancelled={IsCancelled})";
}

public interface ITaskScheduler
{
    TaskHandler RunTask(Plugin.PluginBase plugin, Action action);
    TaskHandler RunTaskLater(Plugin.PluginBase plugin, Action action, int delayTicks);
    TaskHandler RunTaskTimer(Plugin.PluginBase plugin, Action action, int delayTicks, int periodTicks);
    void CancelTask(TaskHandler handler);
    void CancelTasks(Plugin.PluginBase plugin);
    int PendingTasks { get; }
}
