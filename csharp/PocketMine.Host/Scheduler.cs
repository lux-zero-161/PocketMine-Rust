using PocketMine.Plugin;
using PocketMine.Scheduler;

namespace PocketMine.Host;

/// <summary>Tick-driven scheduler. Rust calls <see cref="Tick"/> once per server tick.</summary>
public sealed class HostTaskScheduler : ITaskScheduler
{
    private sealed class ScheduledTask
    {
        public required TaskHandler Handler { get; init; }
        public required PluginBase Plugin { get; init; }
        public required Action Action { get; init; }
        public long NextTick { get; set; }
        public int Period { get; set; }
        public bool Cancelled { get; set; }
    }

    private readonly Dictionary<int, ScheduledTask> _tasks = new();
    private int _nextId = 1;

    public int PendingTasks
    {
        get
        {
            lock (_tasks)
            {
                return _tasks.Count;
            }
        }
    }

    public TaskHandler RunTask(PluginBase plugin, Action action) => RunTaskLater(plugin, action, 1);

    public TaskHandler RunTaskLater(PluginBase plugin, Action action, int delayTicks)
    {
        lock (_tasks)
        {
            int id = _nextId++;
            var handler = new TaskHandler
            {
                Id = id,
                PluginName = plugin.Name,
                IsRepeating = false,
            };
            handler.CancelAction = () =>
            {
                lock (_tasks)
                {
                    _tasks.Remove(id);
                }
            };
            _tasks[id] = new ScheduledTask
            {
                Handler = handler,
                Plugin = plugin,
                Action = action,
                NextTick = CurrentTick + System.Math.Max(1, delayTicks),
                Period = 0,
            };
            return handler;
        }
    }

    public TaskHandler RunTaskTimer(PluginBase plugin, Action action, int delayTicks, int periodTicks)
    {
        lock (_tasks)
        {
            int id = _nextId++;
            var handler = new TaskHandler
            {
                Id = id,
                PluginName = plugin.Name,
                IsRepeating = true,
            };
            handler.CancelAction = () =>
            {
                lock (_tasks)
                {
                    _tasks.Remove(id);
                }
            };
            _tasks[id] = new ScheduledTask
            {
                Handler = handler,
                Plugin = plugin,
                Action = action,
                NextTick = CurrentTick + System.Math.Max(1, delayTicks),
                Period = System.Math.Max(1, periodTicks),
            };
            return handler;
        }
    }

    public void CancelTask(TaskHandler handler)
    {
        handler.Cancel();
    }

    public void CancelTasks(PluginBase plugin)
    {
        lock (_tasks)
        {
            var ids = _tasks.Where(kv => ReferenceEquals(kv.Value.Plugin, plugin) || kv.Value.Plugin.Name == plugin.Name)
                .Select(kv => kv.Key)
                .ToList();
            foreach (int id in ids)
            {
                _tasks.Remove(id);
            }
        }
    }

    public long CurrentTick { get; private set; }

    public void Tick(ulong tick)
    {
        CurrentTick = (long)tick;
        List<ScheduledTask> due;
        lock (_tasks)
        {
            due = _tasks.Values.Where(t => !t.Cancelled && t.NextTick <= CurrentTick).ToList();
        }

        foreach (ScheduledTask task in due)
        {
            if (task.Cancelled || !task.Plugin.IsEnabled)
            {
                continue;
            }
            try
            {
                task.Action();
            }
            catch (Exception ex)
            {
                Entrypoint.Log(4, $"[{task.Plugin.Name}] Unhandled error in scheduled task #{task.Handler.Id}: {ex}");
            }

            lock (_tasks)
            {
                if (!_tasks.TryGetValue(task.Handler.Id, out ScheduledTask? live) || live.Cancelled)
                {
                    continue;
                }
                if (live.Period > 0)
                {
                    live.NextTick = CurrentTick + live.Period;
                }
                else
                {
                    live.Cancelled = true;
                    live.Handler.IsCancelled = true;
                    _tasks.Remove(live.Handler.Id);
                }
            }
        }
    }
}
