use axum::{
    extract::Query,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BinaryHeap,
    fs::{self, File},
    io::Write,
    net::SocketAddr,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::{net::TcpListener, task};

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
        Queue {
            tasks: BinaryHeap::new(),
        }
    }
}

impl Queue {
    fn run_next_task(&mut self, max_retries: u32) {
        let mut remaining = BinaryHeap::new();

        while let Some(mut app) = self.tasks.pop() {
            if app.status == "queued" {
                app.status = "running".into();
                app.start_time = Some(Utc::now().to_rfc3339());

                for attempt in 1..=max_retries {
                    let output = if cfg!(target_os = "windows") {
                        Command::new("cmd").args(["/C", &app.command]).output()
                    } else {
                        Command::new("bash").arg("-c").arg(&app.command).output()
                    };

                    match output {
                        Ok(ref output) if output.status.success() => {
                            app.status = "completed".into();
                            app.end_time = Some(Utc::now().to_rfc3339());
                            println!("[✓] {} ran successfully.", app.name);
                            break;
                        }
                        Ok(output) => {
                            println!(
                                "[x] {} failed (attempt {}/{}): {}",
                                app.name,
                                attempt,
                                max_retries,
                                String::from_utf8_lossy(&output.stderr)
                            );
                            app.retries += 1;
                            thread::sleep(Duration::from_secs(5));
                        }
                        Err(e) => {
                            println!(
                                "[x] {} failed to start (attempt {}/{}): {}",
                                app.name, attempt, max_retries, e
                            );
                            app.retries += 1;
                            thread::sleep(Duration::from_secs(5));
                        }
                    }
                }

                if app.status != "completed" {
                    app.status = "failed".into();
                    app.end_time = Some(Utc::now().to_rfc3339());
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

#[tokio::main]
async fn main() {
    let queue_file = "queue.yml";
    let queue: SharedQueue = Arc::new(Mutex::new(Queue::load_from_file(queue_file)));

    // 👷 Background task runner
    {
        let queue = Arc::clone(&queue);
        let file = queue_file.to_string();
        task::spawn_blocking(move || loop {
            {
                let mut q = queue.lock().unwrap();
                q.run_next_task(3);
                q.save_to_file(&file);
            }
            thread::sleep(Duration::from_secs(10));
        });
    }

    // 📡 HTTP routes
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

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await.unwrap();
    println!("🚀 Server running on http://{}", addr);

    axum::serve(listener, app).await.unwrap();
}

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
