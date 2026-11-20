namespace PocketMine.Event;

public enum EventPriority
{
    Lowest = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Highest = 4,
    Monitor = 5,
}

public interface IListener
{
}

[AttributeUsage(AttributeTargets.Method, Inherited = false, AllowMultiple = false)]
public sealed class EventHandlerAttribute : Attribute
{
    public EventPriority Priority { get; init; } = EventPriority.Normal;
    public bool IgnoreCancelled { get; init; } = false;
}

public abstract class Event
{
    public virtual bool IsCancelled { get; set; } = false;
}

public abstract class CancellableEvent : Event
{
}
