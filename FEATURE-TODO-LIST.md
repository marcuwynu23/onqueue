# 🛠 Onqueue - Feature To-Do List

A list of planned improvements and ideas for enhancing the Onqueue task queue system.

---

## ✅ Core Improvements

- [ ] Add structured logging (use `tracing` crate)
- [ ] Support CLI command `onqueue add .` to read from `queue-app.yml`
- [ ] Support CLI command `onqueue list` to list like `pm2 ls` style
- [ ] Persist queue state to `queue.yml` on every update
- [ ] Add `/status` endpoint to show queue stats
- [ ] Add `/run?name=...` to trigger specific job by name

---

## 🚀 New Features

- [ ] **Add `/status/:name` endpoint**  
       View the current status of a specific task by name.

- [ ] **Retry failed tasks manually**  
       Expose an endpoint like `/retry?name=...` to re-queue failed tasks.

- [ ] **Delete a task**  
       Add `/delete?name=...` to remove a task from the queue (if not running).

- [ ] **Pause/resume scheduler**  
       Add support to temporarily stop task execution (but keep accepting new tasks).

- [ ] **UI Dashboard**  
       Web interface for adding, viewing, retrying, or deleting tasks.

---

## 🧪 Testing

- [ ] Add integration tests for `/add`, `/list`, `/status`
- [ ] Add unit tests for queue operations (push, pop, sort)

---

## 🔒 Security & Auth

- [ ] Protect endpoints with basic API key or JWT
- [ ] Rate-limit `/add` to prevent abuse

---

## 🐳 Deployment

- [ ] Add Dockerfile and `docker-compose.yml`
- [ ] Add systemd or Windows service support

---

## 🧹 Maintenance

- [ ] Clean up old tasks after a configurable time (e.g., 7 days)
- [ ] Archive or compress old logs

---

## 🧩 Nice to Have

- [ ] CLI client for managing tasks
- [ ] Task tagging & filtering in `/list`
