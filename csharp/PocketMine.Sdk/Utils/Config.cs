namespace PocketMine.Utils;

/// <summary>Simple flat YAML configuration, compatible with PocketMine's config.yml files.</summary>
public sealed class Config
{
    private readonly Dictionary<string, string> _values = new(StringComparer.OrdinalIgnoreCase);
    private string _path = string.Empty;

    public static Config Load(string path)
    {
        var config = new Config { _path = path };
        if (!File.Exists(path))
        {
            return config;
        }
        foreach (string raw in File.ReadAllLines(path))
        {
            string line = raw.TrimEnd();
            if (line.Length == 0 || line.TrimStart().StartsWith('#'))
            {
                continue;
            }
            int colon = line.IndexOf(':');
            if (colon <= 0)
            {
                continue;
            }
            string key = line[..colon].Trim();
            string value = line[(colon + 1)..].Trim().Trim('"').Trim('\'');
            if (value.Length > 0)
            {
                config._values[key] = value;
            }
        }
        return config;
    }

    public Config Set(string key, object? value)
    {
        _values[key] = value?.ToString() ?? "";
        return this;
    }

    public string GetString(string key, string fallback = "") => _values.TryGetValue(key, out string? v) ? v : fallback;

    public bool GetBool(string key, bool fallback = false)
        => _values.TryGetValue(key, out string? v) ? v.Equals("true", StringComparison.OrdinalIgnoreCase) || v == "1" || v.Equals("on", StringComparison.OrdinalIgnoreCase) : fallback;

    public int GetInt(string key, int fallback = 0) => _values.TryGetValue(key, out string? v) && int.TryParse(v, out int i) ? i : fallback;

    public double GetDouble(string key, double fallback = 0) => _values.TryGetValue(key, out string? v) && double.TryParse(v, out double d) ? d : fallback;

    public bool Has(string key) => _values.ContainsKey(key);

    public IReadOnlyDictionary<string, string> Values => _values;

    public void Save(string? path = null)
    {
        path ??= _path;
        if (string.IsNullOrEmpty(path))
        {
            return;
        }
        Directory.CreateDirectory(Path.GetDirectoryName(path) ?? ".");
        File.WriteAllLines(path, _values.Select(kv => $"{kv.Key}: {kv.Value}"));
    }
}
