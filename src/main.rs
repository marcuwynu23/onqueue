use std::{
    collections::BinaryHeap,
    fs::File,
    io::Write,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tiny_http::{Response, Server};

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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Queue {
    tasks: BinaryHeap<App>,
}

impl Queue {
    fn new() -> Self {
        Queue {
            tasks: BinaryHeap::new(),
        }
    }

    fn add_task(&mut self, app_name: &str, command: &str) {
        let app = App {
            name: app_name.to_string(),
            command: command.to_string(),
            status: "queued".to_string(),
            start_time: None,
            end_time: None,
            error_message: None,
            retries: 0,
        };
        self.tasks.push(app);
    }
    fn list_tasks(&self) -> String {
        let mut output = String::new();
        for app in &self.tasks {
            output.push_str(&format!(
                "[{}] {}\n  - status: {}\n  - start: {}\n  - end: {}\n\n",
                app.name,
                app.command,
                app.status,
                app.start_time.clone().unwrap_or("N/A".into()),
                app.end_time.clone().unwrap_or("N/A".into()),
            ));
        }
        if output.is_empty() {
            output.push_str("Queue is empty.\n");
        }
        output
    }

    fn run_next_task(&mut self, max_retries: u32) {
        let mut remaining = BinaryHeap::new();

        while let Some(mut app) = self.tasks.pop() {
            if app.status == "queued" {
                app.status = "running".to_string();
                app.start_time = Some(Utc::now().to_rfc3339());

                for attempt in 1..=max_retries {
                    let output = if cfg!(target_os = "windows") {
                        Command::new("cmd").args(&["/C", &app.command]).output()
                    } else {
                        Command::new("bash").arg("-c").arg(&app.command).output()
                    };

                    match output {
                        Ok(ref output) if output.status.success() => {
                            app.status = "completed".to_string();
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
                    app.status = "failed".to_string();
                    app.end_time = Some(Utc::now().to_rfc3339());
                }
            }
            remaining.push(app);
        }

        self.tasks = remaining;
    }

    fn save(&self, filename: &str) {
        if let Ok(yaml) = serde_yaml::to_string(self) {
            if let Ok(mut file) = File::create(filename) {
                let _ = file.write_all(yaml.as_bytes());
            }
        }
    }

    fn load(filename: &str) -> Self {
        File::open(filename)
            .ok()
            .and_then(|file| serde_yaml::from_reader(file).ok())
            .unwrap_or_else(Queue::new)
    }
}

fn main() {
    let queue_file = "queue.yml";
    let queue = Arc::new(Mutex::new(Queue::load(queue_file)));

    // 👀 Watcher thread that continuously runs tasks
    {
        let queue = Arc::clone(&queue);
        let filename = queue_file.to_string();
        thread::spawn(move || {
            loop {
                {
                    let mut q = queue.lock().unwrap();
                    q.run_next_task(3);
                    q.save(&filename);
                }
                thread::sleep(Duration::from_secs(10));
            }
        });
    }

    // 🌐 Tiny HTTP server
    let server = Server::http("0.0.0.0:8080").unwrap();
    println!("🚀 Server running on http://localhost:8080");

    for request in server.incoming_requests() {
        let response = match request.url() {
            "/" => Response::from_string(
                "Onqueue is running.\nTry:\n  - /add?name=test&cmd=echo+hello\n  - /list",
            ),

            path if path.starts_with("/add") => {
                let query = path.splitn(2, '?').nth(1);
                let mut name = None;
                let mut cmd = None;

                if let Some(q) = query {
                    for (key, value) in url::form_urlencoded::parse(q.as_bytes()) {
                        match key.as_ref() {
                            "name" => name = Some(value.into_owned()),
                            "cmd" => cmd = Some(value.into_owned()),
                            _ => {}
                        }
                    }
                }

                match (name, cmd) {
                    (Some(n), Some(c)) => {
                        let mut q = queue.lock().unwrap();
                        q.add_task(&n, &c);
                        q.save(queue_file);
                        Response::from_string(format!("Queued '{}': `{}`", n, c))
                    }
                    _ => Response::from_string("Missing `name` or `cmd` query parameters")
                        .with_status_code(400),
                }
            }

            "/list" => {
                let q = queue.lock().unwrap();
                Response::from_string(q.list_tasks())
            }

            _ => Response::from_string("404 Not Found").with_status_code(404),
        };

        let _ = request.respond(response);
    }
}
