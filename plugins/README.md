# plugins/

Compiled C# plugins live here.

Two layouts are supported:

```text
plugins/
├── MyPlugin.dll              # single assembly
└── AnotherPlugin/
    ├── AnotherPlugin.dll     # assembly + private dependencies
    └── plugin.json           # optional manifest
```

`plugin.json` fields: `name`, `version`, `main` (fully qualified class name),
`api`, `author`, `description`, `depend` (array of plugin names).

Example:

```json
{
  "name": "AnotherPlugin",
  "version": "1.0.0",
  "main": "AnotherPlugin.Main",
  "api": "1.0.0.0",
  "author": "You",
  "description": "Does something useful",
  "depend": ["ExamplePlugin"]
}
```

The server creates a data folder per plugin (`plugins/<Name>/`) for
`config.yml` and other runtime files. Do not put source code here; build
plugins from `csharp/plugins/` or your own repository.

See `../README.md` and `../PORTING.md` for the C# API and build instructions.
