# LocalDeploy engine — protocol version 1

Transport: JSON-RPC 2.0, one UTF-8 object per line over stdin/stdout. No HTTP port, listening socket, shared server, WebView2 or browser is needed for the transport. stdout contains protocol only. stderr is reserved for diagnostics. Launch with an argument array, never shell interpolation:

`localdeploy-engine --data-dir <absolute path owned by the IDE>`

The command is for the IDE host; the end user never types it. On Windows launch with `windowsHide: true` or `CREATE_NO_WINDOW`. An engine process owns one data directory and can manage multiple workspaces. Do not share a database concurrently with another engine instance.

Request:
```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
```
Response:
```json
{"jsonrpc":"2.0","id":1,"result":{"protocol_version":1}}
```
The actual initialize response also includes version, capabilities and data_directory. Each request must include an id. JSON-RPC batches and client notifications are not implemented. Malformed messages receive errors without ending the session.

| Method | Parameters | Result |
|---|---|---|
| initialize | `{}` | Protocol version and capabilities |
| projects.attach | `path`, optional `branch`, `repo_url` | Project; idempotent for the same absolute repository root |
| projects.list | `{}` | Projects |
| commits.list | `project_id`, optional `limit` (1–500) | SHA, subject, author, date, local/branch/main location |
| projects.sync | `project_id` | `job_id`; fetch origin |
| environment.inspect | `project_id` | Static stack, backend and unsupported-stack blocker |
| builds.plan | `project_id`, `commit` | Proposed preparation plan |
| builds.prepare | `project_id`, `commit`, `trusted:true`, optional `run_tests` | `job_id` |
| builds.list | `project_id` | Persisted build records |
| previews.open | `project_id`, `build_id` | `job_id`; approved artifact only |
| previews.stop | `project_id`, `build_id` | `job_id` |
| previews.remove | `project_id`, `build_id` | `job_id`; delete reserved workspace, never source repository |
| jobs.get | `job_id` | Job snapshot |
| jobs.cancel | `job_id` | Cancel preparation; later observe terminal status |
| logs.read | `project_id`, `build_id` | `{text}` with existing masking |
| settings.read | `{}` | Runtime preferences |
| settings.write | Preferences object | Validated saved preferences; applies to later operations |
| variables.write | `project_id`, `values` as `NAME=value` lines | Protected variables (Windows DPAPI only) |
| environment.prepare | `trusted:true` | `job_id`; Windows prerequisite installer |
| runner.workflow.generate | `project_id`, `trusted:true` | Generated file; commit/push remains an IDE action |
| shutdown | `{}` | Exit if no active jobs; existing previews continue |

Long operations run in workers. Only one job per project is allowed at once; independent projects can progress concurrently. While a build runs, queries remain available. Return from builds.prepare is not proof of a successful build.

Event:
```json
{"jsonrpc":"2.0","method":"localdeploy/event","params":{"type":"job.updated","job":{"id":1,"project_id":1,"kind":"prepare","status":"running","progress":"npm ci","result":null,"error":null}}}
```

States: `running`, `cancelling`, `succeeded`, `failed`, `cancelled`. Query jobs.get after subscribing; events can arrive before the request response. Jobs live in memory; build/log history persists. Completed preparation returns a Build. Opening a preview returns a Build with `endpoint` for web or `endpoint:null` and a native process for desktop. The IDE opens the URL or displays “desktop application launched”. There is no automatic browser launch.

Errors: -32700 invalid JSON; -32600 invalid request; -32601 unknown method; -32602 invalid parameters/trust; -32004 missing project/build/job; -32009 project busy; -32000 backend failure. Error messages explain missing tooling or unsupported requirements. Do not interpret an absent test script as tests passed.

Build cancellation is cooperative: install/build/test commands are interrupted, while initial Git preparation may complete first. Sync, environment installation and preview startup do not support cancellation through jobs.cancel. Wait for those jobs before shutdown. Previews are preserved after shutdown for reconnect; explicitly stop/remove them when closing the IDE if desired. If a host crashes, retained data lets it list and clean previews on next launch.

Source workspaces remain owned by the IDE. Only committed snapshots are tested; uncommitted editor changes are not included. Existing Git credentials, Docker daemon and toolchains are reused. Never send tokens to the embedded preview page or log request payloads containing variables.
