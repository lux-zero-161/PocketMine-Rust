namespace PocketMine.Math;

public readonly struct Vector3 : IEquatable<Vector3>
{
    public double X { get; }
    public double Y { get; }
    public double Z { get; }

    public static readonly Vector3 Zero = new(0, 0, 0);

    public Vector3(double x, double y, double z)
    {
        X = x;
        Y = y;
        Z = z;
    }

    public double Distance(Vector3 other) => System.Math.Sqrt(DistanceSquared(other));

    public double DistanceSquared(Vector3 other)
    {
        double dx = X - other.X;
        double dy = Y - other.Y;
        double dz = Z - other.Z;
        return dx * dx + dy * dy + dz * dz;
    }

    public static Vector3 operator +(Vector3 a, Vector3 b) => new(a.X + b.X, a.Y + b.Y, a.Z + b.Z);
    public static Vector3 operator -(Vector3 a, Vector3 b) => new(a.X - b.X, a.Y - b.Y, a.Z - b.Z);
    public static Vector3 operator *(Vector3 a, double scalar) => new(a.X * scalar, a.Y * scalar, a.Z * scalar);
    public static Vector3 operator /(Vector3 a, double scalar) => new(a.X / scalar, a.Y / scalar, a.Z / scalar);

    public bool Equals(Vector3 other) => X.Equals(other.X) && Y.Equals(other.Y) && Z.Equals(other.Z);
    public override bool Equals(object? obj) => obj is Vector3 other && Equals(other);
    public override int GetHashCode() => HashCode.Combine(X, Y, Z);
    public override string ToString() => $"Vector3({X:F2}, {Y:F2}, {Z:F2})";
}
