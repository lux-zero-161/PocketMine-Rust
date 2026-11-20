namespace PocketMine.Math;

public readonly struct Vector3 : IEquatable<Vector3>
{
    public double X { get; }
    public double Y { get; }
    public double Z { get; }

    public static readonly Vector3 Zero = new(0, 0, 0);
    public static readonly Vector3 One = new(1, 1, 1);

    public Vector3(double x, double y, double z)
    {
        X = x;
        Y = y;
        Z = z;
    }

    public double Length => System.Math.Sqrt(X * X + Y * Y + Z * Z);
    public double LengthSquared => X * X + Y * Y + Z * Z;
    public double Distance(Vector3 other) => System.Math.Sqrt(DistanceSquared(other));

    public double DistanceSquared(Vector3 other)
    {
        double dx = X - other.X;
        double dy = Y - other.Y;
        double dz = Z - other.Z;
        return dx * dx + dy * dy + dz * dz;
    }

    public Vector3 Add(double x, double y, double z) => new(X + x, Y + y, Z + z);
    public Vector3 Floor() => new(System.Math.Floor(X), System.Math.Floor(Y), System.Math.Floor(Z));

    public static Vector3 operator +(Vector3 a, Vector3 b) => new(a.X + b.X, a.Y + b.Y, a.Z + b.Z);
    public static Vector3 operator -(Vector3 a, Vector3 b) => new(a.X - b.X, a.Y - b.Y, a.Z - b.Z);
    public static Vector3 operator *(Vector3 a, double scalar) => new(a.X * scalar, a.Y * scalar, a.Z * scalar);
    public static Vector3 operator /(Vector3 a, double scalar) => new(a.X / scalar, a.Y / scalar, a.Z / scalar);

    public bool Equals(Vector3 other) => X.Equals(other.X) && Y.Equals(other.Y) && Z.Equals(other.Z);
    public override bool Equals(object? obj) => obj is Vector3 other && Equals(other);
    public override int GetHashCode() => HashCode.Combine(X, Y, Z);
    public override string ToString() => $"Vector3({X:F2}, {Y:F2}, {Z:F2})";
}

public readonly struct BlockPosition : IEquatable<BlockPosition>
{
    public int X { get; }
    public int Y { get; }
    public int Z { get; }

    public static readonly BlockPosition Zero = new(0, 0, 0);

    public BlockPosition(int x, int y, int z)
    {
        X = x;
        Y = y;
        Z = z;
    }

    public Vector3 ToVector3() => new(X, Y, Z);
    public double DistanceSquared(BlockPosition other)
    {
        double dx = X - other.X, dy = Y - other.Y, dz = Z - other.Z;
        return dx * dx + dy * dy + dz * dz;
    }

    public bool Equals(BlockPosition other) => X == other.X && Y == other.Y && Z == other.Z;
    public override bool Equals(object? obj) => obj is BlockPosition other && Equals(other);
    public override int GetHashCode() => HashCode.Combine(X, Y, Z);
    public override string ToString() => $"({X}, {Y}, {Z})";
}
