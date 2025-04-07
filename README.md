<div align="center">
  <h1> Onqueue </h1>
</div>

<p align="center">
  <img src="https://img.shields.io/github/stars/marcuwynu23/onqueue.svg" alt="Stars Badge"/>
  <img src="https://img.shields.io/github/forks/marcuwynu23/onqueue.svg" alt="Forks Badge"/>
  <img src="https://img.shields.io/github/issues/marcuwynu23/onqueue.svg" alt="Issues Badge"/>
  <img src="https://img.shields.io/github/license/marcuwynu23/onqueue.svg" alt="License Badge"/>
</p>

**Onqueue** is a lightweight, multithreaded task queue runner built in Rust using [Axum](https://github.com/tokio-rs/axum). It supports REST API and CLI-based task management, making it ideal for automating shell commands, deployment tasks, and lightweight job queues.

---

## 📦 Features

- ✅ Queue tasks with names and commands
- ✅ Web server using Axum with endpoints to add/list tasks
- ✅ CLI support: `onqueue add`, `onqueue list`
- ✅ Multithreaded task runner with retry support
- ✅ Persistent queue file via `queue.yml`
- ✅ JSON API output
- ✅ Automatic retries on failure
- ✅ Configurable apps via `queue-app.yml`

---

## 🧰 Usage

### ▶️ Running the Server

```bash
onqueue serve
```

Server starts on [http://localhost:8080](http://localhost:8080)

---

### 🌐 API Endpoints

- **GET /** – Show welcome message
- **GET /list** – Return current tasks as JSON
- **GET /add?name=app1&cmd=echo+Hello** – Queue a new task

---

### 🖥️ CLI Usage

#### Add from `queue-app.yml`

```yaml
# queue-app.yml
name: deploy
command: ansible-playbook deploy.yml
```

```bash
onqueue add .
```

#### List tasks

```bash
onqueue list
```

---

## 📂 Directory Structure

```
.
├── src/
├── queue.yml           # Stores all queued tasks
├── queue-app.yml       # CLI-based task configuration
├── logs/               # (planned) Directory for task execution logs
├── Cargo.toml
└── README.md
```

---

## 📖 Example

```bash
curl "http://localhost:8080/add?name=build&cmd=echo+Building"
curl "http://localhost:8080/list"
```

Output:

```json
[
  {
    "name": "build",
    "command": "echo Building",
    "status": "completed",
    "start_time": "2025-04-07T10:00:00Z",
    "end_time": "2025-04-07T10:00:01Z",
    "retries": 0
  }
]
```

---

## 🛣 Roadmap

See [FEATURE-TODO-LIST.md](./FEATURE-TODO-LIST.md) for upcoming improvements:

- Logging
- CLI formatting
- Cron-like scheduling
- Persistent task results
- PM2 integration

---

## 🧪 Development

Install dependencies:

```bash
cargo install --path .
```

Run in dev mode:

```bash
cargo run
```

---

## ⚖️ License

MIT © [Your Name or Org]
