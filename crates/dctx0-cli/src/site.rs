use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use clap::{Args, Subcommand, ValueEnum};
use dctx0_engine::{Project, SiteProgress, site_html_json, site_json_with_progress};
use dctx0_store::{SiteHistory, SiteSelection};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
mod progress;
use progress::Progress;
#[derive(Args)]
pub(crate) struct SiteArgs {
    /// Print preparation phases, item counts and elapsed time to stderr.
    #[arg(long, global = true)]
    progress: bool,
    /// Embedded source history; the selected task plan's pinned sources are always included.
    #[arg(long, global = true, value_enum, default_value = "previous")]
    history: History,
    #[command(subcommand)]
    command: SiteCommand,
}
#[derive(Clone, Copy, Default, ValueEnum)]
enum History {
    /// Current and previous distinct snapshots, plus the selected task plan's sources.
    #[default]
    Previous,
    /// Also embed all original snapshots referenced by historical tasks.
    All,
}
impl From<History> for SiteHistory {
    fn from(value: History) -> Self {
        match value {
            History::Previous => Self::Previous,
            History::All => Self::All,
        }
    }
}
#[derive(Subcommand)]
enum SiteCommand {
    /// Write a portable, self-contained read-only companion.
    Export {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        snapshot: Option<String>,
        #[arg(long)]
        plan: Option<i64>,
        #[arg(long)]
        as_of: Option<i64>,
    },
    /// Serve the read-only companion on loopback. Port 0 chooses a free port.
    Serve {
        input: PathBuf,
        #[arg(long, default_value = "0")]
        port: u16,
    },
}
#[derive(Clone)]
struct App {
    project: Arc<Project>,
    cache: Arc<Mutex<Option<CachedSite>>>,
    progress: bool,
    history: History,
}
struct CachedSite {
    revision: dctx0_store::SiteRevision,
    json: Bytes,
    html: Bytes,
    json_etag: String,
    html_etag: String,
}
fn payload(
    app: &App,
    html: bool,
    conditional: Option<&str>,
) -> Result<(Option<Bytes>, String), String> {
    let revision = app.project.site_revision().map_err(crate::error_message)?;
    let mut guard = app
        .cache
        .lock()
        .map_err(|_| "Site cache unavailable".to_string())?;
    if guard
        .as_ref()
        .is_none_or(|cache| cache.revision != revision)
    {
        // Release an obsolete large payload before constructing its replacement.
        *guard = None;
        let mut progress = Progress::new(app.progress);
        progress.report(SiteProgress::LoadingRecords);
        let data = app
            .project
            .site_snapshot(&SiteSelection {
                history: app.history.into(),
                ..Default::default()
            })
            .map_err(crate::error_message)?;
        let json = site_json_with_progress(&data, &mut |event| progress.report(event))
            .map_err(crate::error_message)?;
        let revision = data.records.revision.clone();
        drop(data);
        progress.report(SiteProgress::RenderingHtml);
        let html = site_html_json(&json, true);
        progress.message("Computing HTTP cache hashes");
        *guard = Some(CachedSite {
            revision,
            json_etag: format!("\"{:x}\"", Sha256::digest(json.as_bytes())),
            html_etag: format!("\"{:x}\"", Sha256::digest(html.as_bytes())),
            json: Bytes::from(json),
            html: Bytes::from(html),
        });
        progress.message("HTTP payload prepared");
    }
    let cache = guard.as_ref().ok_or("Site cache unavailable")?;
    let (body, etag) = if html {
        (&cache.html, &cache.html_etag)
    } else {
        (&cache.json, &cache.json_etag)
    };
    Ok((
        if conditional == Some(etag.as_str()) {
            None
        } else {
            Some(body.clone())
        },
        etag.clone(),
    ))
}
async fn respond(app: App, headers: HeaderMap, html: bool) -> Response {
    let conditional = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|h| h.to_str().ok())
        .map(str::to_owned);
    let result =
        tokio::task::spawn_blocking(move || payload(&app, html, conditional.as_deref())).await;
    match result {
        Ok(Ok((body, etag))) => {
            let mut response = body.map_or_else(
                || StatusCode::NOT_MODIFIED.into_response(),
                IntoResponse::into_response,
            );
            response
                .headers_mut()
                .insert(header::ETAG, etag.parse().expect("hex ETag"));
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                if html {
                    "text/html; charset=utf-8"
                } else {
                    "application/json"
                }
                .parse()
                .unwrap(),
            );
            response
                .headers_mut()
                .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
            response
        }
        Ok(Err(error)) => (StatusCode::SERVICE_UNAVAILABLE, error).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response(),
    }
}
async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    respond(app, headers, true).await
}
async fn api(State(app): State<App>, headers: HeaderMap) -> Response {
    respond(app, headers, false).await
}
pub(crate) fn run(args: SiteArgs, db: Option<&Path>, output: &mut dyn Write) -> Result<(), String> {
    let mut progress = Progress::new(args.progress);
    let result = run_command(args, db, output, &mut progress);
    if let Err(error) = &result {
        progress.message(&format!("FAILED: {error}"));
    }
    result
}
fn run_command(
    args: SiteArgs,
    db: Option<&Path>,
    output: &mut dyn Write,
    progress: &mut Progress,
) -> Result<(), String> {
    match args.command {
        SiteCommand::Export {
            input,
            output: destination,
            snapshot,
            plan,
            as_of,
        } => {
            let project = Project::new(&input, db).map_err(crate::error_message)?;
            project
                .site_export_with_progress(
                    &SiteSelection {
                        snapshot,
                        plan,
                        as_of,
                        history: args.history.into(),
                    },
                    &destination,
                    &mut |event| progress.report(event),
                )
                .map_err(crate::error_message)?;
            writeln!(
                output,
                "Exported {}",
                destination.join("index.html").display()
            )
            .map_err(crate::error_message)?;
            output.flush().map_err(crate::error_message)?;
            progress.message("READY: export written");
            Ok(())
        }
        SiteCommand::Serve { input, port } => {
            let project = Project::new(&input, db).map_err(crate::error_message)?;
            let app = App {
                project: Arc::new(project),
                cache: Arc::new(Mutex::new(None)),
                progress: args.progress,
                history: args.history,
            };
            payload(&app, false, None)?;
            let runtime = tokio::runtime::Runtime::new().map_err(crate::error_message)?;
            runtime.block_on(async {
                let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                    .await
                    .map_err(crate::error_message)?;
                writeln!(
                    output,
                    "http://{}",
                    listener.local_addr().map_err(crate::error_message)?
                )
                .map_err(crate::error_message)?;
                output.flush().map_err(crate::error_message)?;
                progress.message(&format!(
                    "READY: http://{} (Ctrl-C to stop)",
                    listener.local_addr().map_err(crate::error_message)?
                ));
                let router = Router::new()
                    .route("/", get(index))
                    .route("/api/site", get(api))
                    .with_state(app);
                axum::serve(listener, router)
                    .with_graceful_shutdown(async {
                        let _ = tokio::signal::ctrl_c().await;
                    })
                    .await
                    .map_err(crate::error_message)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_unpublished_and_poisoned_history_responses_remain_errors() {
        let root =
            std::env::temp_dir().join(format!("dctx0-site-http-errors-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let project = Project::new(&root, None).unwrap();
        let app = App {
            project: Arc::new(project.clone()),
            cache: Arc::new(Mutex::new(None)),
            progress: false,
            history: History::Previous,
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            runtime
                .block_on(respond(app.clone(), HeaderMap::new(), false))
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(!project.database.exists());
        std::fs::create_dir_all(project.database.parent().unwrap()).unwrap();
        drop(
            dctx0_store::Store::open(&project.database, dctx0_store::AccessMode::ReadWrite)
                .unwrap(),
        );
        assert_eq!(
            runtime
                .block_on(respond(app.clone(), HeaderMap::new(), false))
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        std::fs::write(root.join("spec.dctx0"), "workspace W { type A = Int }").unwrap();
        project.check().unwrap();
        let response = runtime.block_on(api(State(app.clone()), HeaderMap::new()));
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        let mut conditional = HeaderMap::new();
        conditional.insert(
            header::IF_NONE_MATCH,
            response.headers()[header::ETAG].clone(),
        );
        let unchanged = runtime.block_on(api(State(app.clone()), conditional));
        assert_eq!(unchanged.status(), StatusCode::NOT_MODIFIED);
        let mut invalid = HeaderMap::new();
        invalid.insert(
            header::IF_NONE_MATCH,
            axum::http::HeaderValue::from_bytes(&[0xff]).unwrap(),
        );
        let html = runtime.block_on(index(State(app.clone()), invalid));
        assert_eq!(html.status(), StatusCode::OK);
        assert_eq!(
            html.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(html.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        let cache = app.cache.clone();
        assert!(
            std::thread::spawn(move || {
                let _guard = cache.lock().unwrap();
                panic!("Injected unavailable cache")
            })
            .join()
            .is_err()
        );
        assert_eq!(
            runtime
                .block_on(respond(app, HeaderMap::new(), false))
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod output_tests {
    use super::*;
    struct RejectOutput {
        accepts_write: bool,
    }
    impl Write for RejectOutput {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.accepts_write {
                Ok(bytes.len())
            } else {
                Err(std::io::Error::other("Output unavailable"))
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("Flush unavailable"))
        }
    }
    #[test]
    fn export_output_failure_busy_ports_and_server_flush_errors_are_reported() {
        let root =
            std::env::temp_dir().join(format!("dctx0-site-output-errors-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("spec.dctx0"), "workspace W { type A = Int }").unwrap();
        Project::new(&root, None).unwrap().check().unwrap();
        let destination = root.join("export");
        assert!(
            run(
                SiteArgs {
                    progress: false,
                    history: History::Previous,
                    command: SiteCommand::Export {
                        input: root.clone(),
                        output: destination.clone(),
                        snapshot: None,
                        plan: None,
                        as_of: None
                    }
                },
                None,
                &mut RejectOutput {
                    accepts_write: false
                }
            )
            .unwrap_err()
            .contains("Output unavailable")
        );
        assert!(destination.join("index.html").is_file());
        for accepts_write in [false, true] {
            assert!(
                run(
                    SiteArgs {
                        progress: false,
                        history: History::Previous,
                        command: SiteCommand::Serve {
                            input: root.clone(),
                            port: 0
                        }
                    },
                    None,
                    &mut RejectOutput { accepts_write }
                )
                .is_err()
            );
        }
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(
            run(
                SiteArgs {
                    progress: false,
                    history: History::Previous,
                    command: SiteCommand::Serve {
                        input: root.clone(),
                        port
                    }
                },
                None,
                &mut Vec::new()
            )
            .is_err()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
