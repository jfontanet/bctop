# Docker monitoring and management tool

Terminal UI used to monitor and manage docker containers and services.

# Install:

1. git pull this repo
2. cargo install --path .

# TODOS:
- [X] Detect when a container has been removed
- [X] Differentiate between individual containers, compose services and swarm services (BE)
- [X] Diffetentiate between swarm stack and compose projects (BE)
- [X] Actions per state
- [ ] Monitoring UI
    - [ ] Menu on enter (Pop Up)
- [X] Logging UI
    - [X] Scrol
    - [X] Leave
    - [X] Mouse Scroll
    - [X] Line Jumps
    - [X] Search
- [ ] Exec commands UI
    - [X] Interactive session UI
    - [ ] Input bar (with auto complete?)
