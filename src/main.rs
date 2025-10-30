use axum::{
    Router,
    extract::Query,
    response::{IntoResponse, Json},
    routing::get,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BinaryHeap,
    fs::{self, File, OpenOptions},
    io::Write,
    net::SocketAddr,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::{net::TcpListener, task};

// ---------------- CONFIG ----------------

#[derive(Debug, Deserialize)]
struct Config {
    host: Option<String>,
    port: Option<u16>,
    task_interval: Option<u64>,
    log_file: Option<String>,
}

impl Config {
    fn load_optional(filename: &str) -> Self {
        match fs::read_to_string(filename) {
            Ok(content) => match serde_yaml::from_str::<Config>(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    println!("⚠️  Failed to parse {}: {}. Using defaults.", filename, e);
                    Self::default()
                }
            },
            Err(_) => {
                println!("ℹ️  No config.yml found. Using defaults.");
                Self::default()
            }
        }
    }

    fn address(&self) -> SocketAddr {
        let host = self.host.clone().unwrap_or_else(|| "0.0.0.0".to_string());
        let port = self.port.unwrap_or(8080);
        format!("{}:{}", host, port)
            .parse()
            .expect("Invalid host or port in configuration")
    }

    fn interval(&self) -> u64 {
        self.task_interval.unwrap_or(10)
    }

    fn log_path(&self) -> String {
        self.log_file
            .clone()
            .unwrap_or_else(|| "logs/output.log".to_string())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
            task_interval: None,
            log_file: None,
        }
    }
}

// ---------------- STRUCTS ----------------

#[derive(Debug, Serialize, Deserialize, Eq, Ord, PartialEq, PartialOrd, Clone)]
struct App {
    name: String,
    command: String,
    status: String,
    start_time: Option<String>,
    end_time: Option<String>,
    error_message: Option<String>,
    retries: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Queue {
    tasks: BinaryHeap<App>,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            tasks: BinaryHeap::new(),
        }
    }
}

// ---------------- LOGGING ----------------

fn log_message(path: &str, message: &str) {
    println!("{}", message);
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(file, "[{}] {}", timestamp, message);
    }
}

// ---------------- QUEUE LOGIC ----------------

impl Queue {
    fn run_next_task(&mut self, log_path: &str) {
        let mut remaining = BinaryHeap::new();

        while let Some(mut app) = self.tasks.pop() {
            if app.status == "queued" {
                app.status = "running".into();
                app.start_time = Some(Utc::now().to_rfc3339());
                log_message(log_path, &format!("▶️  Running {}", app.name));

                let is_gui_command = cfg!(target_os = "windows")
                    && (app.command.contains("explorer.exe")
                        || app.command.contains(".exe")
                        || app.command.contains("start "));

                if is_gui_command {
                    let result = Command::new("cmd").args(["/C", &app.command]).spawn();

                    match result {
                        Ok(_) => {
                            app.status = "completed".into();
                            app.end_time = Some(Utc::now().to_rfc3339());
                            log_message(
                                log_path,
                                &format!("[✓] {} launched successfully.", app.name),
                            );
                        }
                        Err(e) => {
                            app.status = "failed".into();
                            app.end_time = Some(Utc::now().to_rfc3339());
                            app.error_message = Some(format!("Failed to start: {}", e));
                            log_message(
                                log_path,
                                &format!("[x] {} could not start: {}", app.name, e),
                            );
                        }
                    }
                } else {
                    let output = if cfg!(target_os = "windows") {
                        Command::new("cmd").args(["/C", &app.command]).output()
                    } else {
                        Command::new("bash").arg("-c").arg(&app.command).output()
                    };

                    match output {
                        Ok(ref output) if output.status.success() => {
                            app.status = "completed".into();
                            app.end_time = Some(Utc::now().to_rfc3339());
                            log_message(log_path, &format!("[✓] {} ran successfully.", app.name));
                        }
                        Ok(output) => {
                            app.status = "failed".into();
                            app.end_time = Some(Utc::now().to_rfc3339());
                            app.error_message =
                                Some(String::from_utf8_lossy(&output.stderr).to_string());
                            log_message(
                                log_path,
                                &format!(
                                    "[x] {} failed: {}",
                                    app.name,
                                    app.error_message.as_deref().unwrap_or("Unknown error")
                                ),
                            );
                        }
                        Err(e) => {
                            app.status = "failed".into();
                            app.end_time = Some(Utc::now().to_rfc3339());
                            app.error_message = Some(format!("Failed to start: {}", e));
                            log_message(
                                log_path,
                                &format!("[x] {} could not start: {}", app.name, e),
                            );
                        }
                    }
                }
            }

            remaining.push(app);
        }

        self.tasks = remaining;
    }

    fn save_to_file(&self, filename: &str) {
        if let Ok(yaml) = serde_yaml::to_string(self) {
            if let Ok(mut file) = File::create(filename) {
                let _ = file.write_all(yaml.as_bytes());
            }
        }
    }

    fn load_from_file(filename: &str) -> Self {
        if let Ok(content) = fs::read_to_string(filename) {
            serde_yaml::from_str(&content).unwrap_or_default()
        } else {
            Queue::default()
        }
    }
}

type SharedQueue = Arc<Mutex<Queue>>;

// ---------------- MAIN ----------------

#[tokio::main]
async fn main() {
    let config = Config::load_optional("config.yml");
    let log_path = config.log_path();
    let interval = config.interval();

    let queue_file = "records.yml";
    let queue: SharedQueue = Arc::new(Mutex::new(Queue::load_from_file(queue_file)));

    {
        let queue = Arc::clone(&queue);
        let file = queue_file.to_string();
        let log_path = log_path.clone();

        task::spawn_blocking(move || {
            loop {
                {
                    let mut q = queue.lock().unwrap();
                    q.run_next_task(&log_path);
                    q.save_to_file(&file);
                }
                thread::sleep(Duration::from_secs(interval));
            }
        });
    }

    let app = Router::new()
        .route("/", get(root))
        .route(
            "/list",
            get({
                let queue = Arc::clone(&queue);
                move || list_handler(queue)
            }),
        )
        .route(
            "/add",
            get({
                let queue = Arc::clone(&queue);
                move |query| add_handler(query, queue, queue_file.to_string())
            }),
        );

    let addr = config.address();
    let listener = TcpListener::bind(addr).await.unwrap();
    log_message(&log_path, &format!("🚀 Server running on http://{}", addr));

    axum::serve(listener, app).await.unwrap();
}

// ---------------- ROUTES ----------------

async fn root() -> &'static str {
    "Onqueue is running.\nTry:\n  - GET /add?name=job1&cmd=echo+hi\n  - GET /list"
}

async fn list_handler(queue: SharedQueue) -> impl IntoResponse {
    let q = queue.lock().unwrap();
    Json(json!(q.tasks.clone().into_sorted_vec()))
}

#[derive(Debug, Deserialize)]
struct AddParams {
    name: String,
    cmd: String,
}

async fn add_handler(
    Query(params): Query<AddParams>,
    queue: SharedQueue,
    queue_file: String,
) -> impl IntoResponse {
    let mut q = queue.lock().unwrap();
    q.tasks.push(App {
        name: params.name.clone(),
        command: params.cmd.clone(),
        status: "queued".into(),
        start_time: None,
        end_time: None,
        error_message: None,
        retries: 0,
    });
    q.save_to_file(&queue_file);

    Json(json!({ "queued": params.name, "cmd": params.cmd }))
}
