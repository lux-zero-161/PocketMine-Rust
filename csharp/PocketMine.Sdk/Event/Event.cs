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

/// <summary>Marker interface for classes containing event handlers.</summary>
public interface IListener
{
}

[AttributeUsage(AttributeTargets.Method, Inherited = false, AllowMultiple = false)]
public sealed class EventHandlerAttribute : Attribute
{
    public EventPriority Priority { get; init; } = EventPriority.Normal;
    public bool IgnoreCancelled { get; init; }
}

public abstract class Event
{
    public virtual bool IsCancellable => false;
    public bool IsCancelled { get; set; }
    public abstract string EventName { get; }
}

public abstract class CancellableEvent : Event
{
    public override bool IsCancellable => true;
}
