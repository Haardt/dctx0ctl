pub(crate) const MIGRATION:&str="
CREATE TABLE check_renames(check_id INTEGER PRIMARY KEY REFERENCES checks(id),rename_id TEXT NOT NULL UNIQUE REFERENCES renames(id));
CREATE TABLE task_plans(id INTEGER PRIMARY KEY AUTOINCREMENT,project TEXT NOT NULL REFERENCES projects(project),parent_id INTEGER REFERENCES task_plans(id),snapshot_id TEXT NOT NULL REFERENCES snapshots(id),check_run_id INTEGER NOT NULL REFERENCES checks(id),created_seq INTEGER NOT NULL DEFAULT 0,data TEXT NOT NULL);
CREATE INDEX task_plans_project ON task_plans(project,id);
CREATE TABLE task_heads(project TEXT PRIMARY KEY REFERENCES projects(project),plan_id INTEGER NOT NULL REFERENCES task_plans(id));
CREATE TABLE tasks(id INTEGER PRIMARY KEY AUTOINCREMENT,project TEXT NOT NULL REFERENCES projects(project),logical_key TEXT NOT NULL,kind TEXT NOT NULL,requirements_hash TEXT NOT NULL,data TEXT NOT NULL,created_seq INTEGER NOT NULL DEFAULT 0);
CREATE INDEX tasks_subject ON tasks(project,logical_key);
CREATE TABLE task_members(plan_id INTEGER NOT NULL REFERENCES task_plans(id),task_id INTEGER NOT NULL REFERENCES tasks(id),wave INTEGER NOT NULL,PRIMARY KEY(plan_id,task_id));
CREATE TABLE task_dependencies(plan_id INTEGER NOT NULL REFERENCES task_plans(id),task_id INTEGER NOT NULL REFERENCES tasks(id),prerequisite_id INTEGER NOT NULL REFERENCES tasks(id),condition TEXT NOT NULL,reason TEXT NOT NULL);
CREATE INDEX task_dependents ON task_dependencies(plan_id,prerequisite_id);
CREATE TABLE task_links(project TEXT NOT NULL REFERENCES projects(project),from_id INTEGER NOT NULL REFERENCES tasks(id),to_id INTEGER NOT NULL REFERENCES tasks(id),kind TEXT NOT NULL,reason TEXT NOT NULL);
CREATE TABLE task_events(seq INTEGER PRIMARY KEY AUTOINCREMENT,project TEXT NOT NULL REFERENCES projects(project),task_id INTEGER REFERENCES tasks(id),kind TEXT NOT NULL,message TEXT NOT NULL,state_data TEXT,created_at TEXT NOT NULL DEFAULT(strftime('%Y-%m-%dT%H:%M:%fZ','now')));
CREATE INDEX task_events_lookup ON task_events(project,task_id,seq);
CREATE TABLE task_states(task_id INTEGER PRIMARY KEY REFERENCES tasks(id),work TEXT NOT NULL,actor TEXT,version INTEGER NOT NULL,data TEXT NOT NULL);
CREATE INDEX task_states_selection ON task_states(work,actor);
CREATE TABLE task_groups(id INTEGER PRIMARY KEY AUTOINCREMENT,project TEXT NOT NULL REFERENCES projects(project),plan_id INTEGER NOT NULL REFERENCES task_plans(id),actor TEXT NOT NULL,status TEXT NOT NULL,data TEXT NOT NULL);
PRAGMA user_version=2;";

pub(crate) const PROFILE_MIGRATION: &str = "
CREATE TABLE execution_bundles(hash TEXT PRIMARY KEY,data TEXT NOT NULL);
CREATE TABLE task_profile_bindings(event_seq INTEGER PRIMARY KEY REFERENCES task_events(seq),task_id INTEGER NOT NULL REFERENCES tasks(id),bundle_hash TEXT NOT NULL REFERENCES execution_bundles(hash),claim INTEGER,reservation INTEGER);
CREATE TABLE task_profile_updates(hash TEXT PRIMARY KEY,project TEXT NOT NULL REFERENCES projects(project),plan_id INTEGER NOT NULL REFERENCES task_plans(id),data TEXT NOT NULL);
PRAGMA user_version=3;";
