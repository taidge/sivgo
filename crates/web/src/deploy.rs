//! Deployment configuration generators.
//!
//! Produces Docker Compose, Nginx, and GitHub Actions CI configuration
//! files from structured Rust data models.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Docker Compose
// ---------------------------------------------------------------------------

/// A service entry in a Docker Compose file.
#[derive(Debug, Clone)]
pub struct ComposeService {
    pub name: String,
    pub image: String,
    pub ports: Vec<String>,
    pub environment: HashMap<String, String>,
    pub depends_on: Vec<String>,
    pub volumes: Vec<String>,
}

/// Docker Compose configuration for the Sivgo stack.
#[derive(Debug, Clone)]
pub struct DockerComposeConfig {
    pub services: Vec<ComposeService>,
    pub volumes: Vec<String>,
    pub networks: Vec<String>,
}

impl Default for DockerComposeConfig {
    fn default() -> Self {
        let mut postgres_env = HashMap::new();
        postgres_env.insert("POSTGRES_DB".into(), "sivgo".into());
        postgres_env.insert("POSTGRES_USER".into(), "sivgo".into());
        postgres_env.insert("POSTGRES_PASSWORD".into(), "sivgo_password".into());

        let mut server_env = HashMap::new();
        server_env.insert("DATABASE_URL".into(), "postgres://sivgo:sivgo_password@postgres:5432/sivgo".into());
        server_env.insert("HOST".into(), "0.0.0.0".into());
        server_env.insert("PORT".into(), "3000".into());
        server_env.insert("JWT_SECRET".into(), "${JWT_SECRET}".into());

        Self {
            services: vec![
                ComposeService {
                    name: "postgres".into(),
                    image: "postgres:16-alpine".into(),
                    ports: vec!["5432:5432".into()],
                    environment: postgres_env,
                    depends_on: vec![],
                    volumes: vec!["pgdata:/var/lib/postgresql/data".into()],
                },
                ComposeService {
                    name: "server".into(),
                    image: "sivgo-server:latest".into(),
                    ports: vec!["3000:3000".into()],
                    environment: server_env,
                    depends_on: vec!["postgres".into()],
                    volumes: vec![],
                },
                ComposeService {
                    name: "nginx".into(),
                    image: "nginx:alpine".into(),
                    ports: vec!["80:80".into(), "443:443".into()],
                    environment: HashMap::new(),
                    depends_on: vec!["server".into()],
                    volumes: vec![
                        "./nginx.conf:/etc/nginx/conf.d/default.conf:ro".into(),
                        "./certs:/etc/nginx/certs:ro".into(),
                    ],
                },
            ],
            volumes: vec!["pgdata".into()],
            networks: vec!["sivgo-net".into()],
        }
    }
}

impl DockerComposeConfig {
    /// Create a new default Sivgo Docker Compose configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a `docker-compose.yml` string.
    pub fn generate_compose(&self) -> String {
        let mut out = String::from("version: '3.8'\n\nservices:\n");

        for svc in &self.services {
            out.push_str(&format!("  {}:\n", svc.name));
            out.push_str(&format!("    image: {}\n", svc.image));

            if !svc.ports.is_empty() {
                out.push_str("    ports:\n");
                for p in &svc.ports {
                    out.push_str(&format!("      - \"{p}\"\n"));
                }
            }

            if !svc.environment.is_empty() {
                out.push_str("    environment:\n");
                let mut keys: Vec<_> = svc.environment.keys().collect();
                keys.sort();
                for k in keys {
                    out.push_str(&format!("      {}: \"{}\"\n", k, svc.environment[k]));
                }
            }

            if !svc.depends_on.is_empty() {
                out.push_str("    depends_on:\n");
                for d in &svc.depends_on {
                    out.push_str(&format!("      - {d}\n"));
                }
            }

            if !svc.volumes.is_empty() {
                out.push_str("    volumes:\n");
                for v in &svc.volumes {
                    out.push_str(&format!("      - {v}\n"));
                }
            }

            out.push('\n');
        }

        if !self.volumes.is_empty() {
            out.push_str("volumes:\n");
            for v in &self.volumes {
                out.push_str(&format!("  {v}:\n"));
            }
            out.push('\n');
        }

        if !self.networks.is_empty() {
            out.push_str("networks:\n");
            for n in &self.networks {
                out.push_str(&format!("  {n}:\n"));
            }
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Nginx
// ---------------------------------------------------------------------------

/// Nginx reverse-proxy configuration.
#[derive(Debug, Clone)]
pub struct NginxConfig {
    /// The server name (domain).
    pub server_name: String,
    /// Whether to enable SSL/TLS.
    pub ssl: bool,
    /// The port of the upstream application server.
    pub upstream_port: u16,
}

impl Default for NginxConfig {
    fn default() -> Self {
        Self {
            server_name: "sivgo.app".to_string(),
            ssl: true,
            upstream_port: 3000,
        }
    }
}

impl NginxConfig {
    /// Create a new default Nginx configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate an `nginx.conf` server block.
    pub fn generate_nginx_conf(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!(
            "upstream sivgo_backend {{\n    server server:{};\n}}\n\n",
            self.upstream_port
        ));

        if self.ssl {
            // HTTP -> HTTPS redirect.
            out.push_str("server {\n");
            out.push_str("    listen 80;\n");
            out.push_str(&format!("    server_name {};\n", self.server_name));
            out.push_str("    return 301 https://$host$request_uri;\n");
            out.push_str("}\n\n");
        }

        out.push_str("server {\n");
        if self.ssl {
            out.push_str("    listen 443 ssl http2;\n");
            out.push_str(&format!("    server_name {};\n\n", self.server_name));
            out.push_str("    ssl_certificate /etc/nginx/certs/cert.pem;\n");
            out.push_str("    ssl_certificate_key /etc/nginx/certs/key.pem;\n");
            out.push_str("    ssl_protocols TLSv1.2 TLSv1.3;\n\n");
        } else {
            out.push_str("    listen 80;\n");
            out.push_str(&format!("    server_name {};\n\n", self.server_name));
        }

        // Static files.
        out.push_str("    location / {\n");
        out.push_str("        root /usr/share/nginx/html;\n");
        out.push_str("        try_files $uri $uri/ /index.html;\n");
        out.push_str("    }\n\n");

        // API proxy.
        out.push_str("    location /api/ {\n");
        out.push_str("        proxy_pass http://sivgo_backend;\n");
        out.push_str("        proxy_set_header Host $host;\n");
        out.push_str("        proxy_set_header X-Real-IP $remote_addr;\n");
        out.push_str("        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n");
        out.push_str("        proxy_set_header X-Forwarded-Proto $scheme;\n");
        out.push_str("    }\n\n");

        // WebSocket proxy.
        out.push_str("    location /ws/ {\n");
        out.push_str("        proxy_pass http://sivgo_backend;\n");
        out.push_str("        proxy_http_version 1.1;\n");
        out.push_str("        proxy_set_header Upgrade $http_upgrade;\n");
        out.push_str("        proxy_set_header Connection \"upgrade\";\n");
        out.push_str("    }\n");

        out.push_str("}\n");

        out
    }
}

// ---------------------------------------------------------------------------
// CI / GitHub Actions
// ---------------------------------------------------------------------------

/// A single step in a CI pipeline.
#[derive(Debug, Clone)]
pub struct CiStep {
    pub name: String,
    pub command: String,
    pub env: HashMap<String, String>,
}

/// CI pipeline configuration.
#[derive(Debug, Clone)]
pub struct CiConfig {
    pub steps: Vec<CiStep>,
}

impl Default for CiConfig {
    fn default() -> Self {
        Self {
            steps: vec![
                CiStep {
                    name: "Checkout".into(),
                    command: "actions/checkout@v4".into(),
                    env: HashMap::new(),
                },
                CiStep {
                    name: "Install Rust".into(),
                    command: "dtolnay/rust-toolchain@stable".into(),
                    env: HashMap::new(),
                },
                CiStep {
                    name: "Cache cargo".into(),
                    command: "actions/cache@v4".into(),
                    env: HashMap::new(),
                },
                CiStep {
                    name: "Check formatting".into(),
                    command: "cargo fmt --all -- --check".into(),
                    env: HashMap::new(),
                },
                CiStep {
                    name: "Clippy".into(),
                    command: "cargo clippy --workspace -- -D warnings".into(),
                    env: HashMap::new(),
                },
                CiStep {
                    name: "Run tests".into(),
                    command: "cargo test --workspace".into(),
                    env: HashMap::new(),
                },
                CiStep {
                    name: "Build release".into(),
                    command: "cargo build --release".into(),
                    env: HashMap::new(),
                },
            ],
        }
    }
}

impl CiConfig {
    /// Create a new default CI configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a step to the pipeline.
    pub fn add_step(&mut self, name: &str, command: &str) {
        self.steps.push(CiStep {
            name: name.to_string(),
            command: command.to_string(),
            env: HashMap::new(),
        });
    }

    /// Generate a GitHub Actions workflow YAML string.
    pub fn generate_github_actions(&self) -> String {
        let mut out = String::new();

        out.push_str("name: CI\n\n");
        out.push_str("on:\n");
        out.push_str("  push:\n");
        out.push_str("    branches: [main]\n");
        out.push_str("  pull_request:\n");
        out.push_str("    branches: [main]\n\n");
        out.push_str("jobs:\n");
        out.push_str("  build:\n");
        out.push_str("    runs-on: ubuntu-latest\n");
        out.push_str("    steps:\n");

        for step in &self.steps {
            out.push_str(&format!("      - name: {}\n", step.name));

            // Determine whether this is a `uses:` action or a `run:` command.
            if step.command.contains('/') && !step.command.contains(' ') {
                out.push_str(&format!("        uses: {}\n", step.command));
            } else {
                out.push_str(&format!("        run: {}\n", step.command));
            }

            if !step.env.is_empty() {
                out.push_str("        env:\n");
                let mut keys: Vec<_> = step.env.keys().collect();
                keys.sort();
                for k in keys {
                    out.push_str(&format!("          {}: {}\n", k, step.env[k]));
                }
            }
        }

        out
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_compose_has_three_services() {
        let config = DockerComposeConfig::new();
        assert_eq!(config.services.len(), 3);
        let names: Vec<&str> = config.services.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"postgres"));
        assert!(names.contains(&"server"));
        assert!(names.contains(&"nginx"));
    }

    #[test]
    fn generate_compose_contains_services() {
        let config = DockerComposeConfig::new();
        let yaml = config.generate_compose();
        assert!(yaml.contains("services:"));
        assert!(yaml.contains("postgres:"));
        assert!(yaml.contains("server:"));
        assert!(yaml.contains("nginx:"));
        assert!(yaml.contains("volumes:"));
        assert!(yaml.contains("networks:"));
    }

    #[test]
    fn generate_compose_contains_ports() {
        let config = DockerComposeConfig::new();
        let yaml = config.generate_compose();
        assert!(yaml.contains("5432:5432"));
        assert!(yaml.contains("3000:3000"));
        assert!(yaml.contains("80:80"));
        assert!(yaml.contains("443:443"));
    }

    #[test]
    fn nginx_default_has_ssl() {
        let config = NginxConfig::new();
        assert!(config.ssl);
        assert_eq!(config.server_name, "sivgo.app");
        assert_eq!(config.upstream_port, 3000);
    }

    #[test]
    fn generate_nginx_with_ssl() {
        let config = NginxConfig::new();
        let conf = config.generate_nginx_conf();
        assert!(conf.contains("listen 443 ssl http2"));
        assert!(conf.contains("ssl_certificate"));
        assert!(conf.contains("return 301 https://"));
        assert!(conf.contains("proxy_pass http://sivgo_backend"));
        assert!(conf.contains("proxy_set_header Upgrade"));
    }

    #[test]
    fn generate_nginx_without_ssl() {
        let config = NginxConfig {
            ssl: false,
            ..Default::default()
        };
        let conf = config.generate_nginx_conf();
        assert!(conf.contains("listen 80"));
        assert!(!conf.contains("listen 443"));
        assert!(!conf.contains("ssl_certificate"));
    }

    #[test]
    fn ci_default_steps() {
        let config = CiConfig::new();
        assert_eq!(config.steps.len(), 7);
        assert_eq!(config.steps[0].name, "Checkout");
        assert_eq!(config.steps[5].name, "Run tests");
    }

    #[test]
    fn add_ci_step() {
        let mut config = CiConfig::new();
        config.add_step("Deploy", "deploy.sh");
        assert_eq!(config.steps.len(), 8);
        assert_eq!(config.steps[7].name, "Deploy");
    }

    #[test]
    fn generate_github_actions_yaml() {
        let config = CiConfig::new();
        let yaml = config.generate_github_actions();
        assert!(yaml.contains("name: CI"));
        assert!(yaml.contains("on:"));
        assert!(yaml.contains("push:"));
        assert!(yaml.contains("pull_request:"));
        assert!(yaml.contains("runs-on: ubuntu-latest"));
        assert!(yaml.contains("uses: actions/checkout@v4"));
        assert!(yaml.contains("run: cargo test --workspace"));
    }

    #[test]
    fn ci_step_with_env() {
        let mut config = CiConfig::new();
        let mut env = HashMap::new();
        env.insert("DATABASE_URL".into(), "postgres://localhost/test".into());
        config.steps.push(CiStep {
            name: "Integration tests".into(),
            command: "cargo test --features integration".into(),
            env,
        });
        let yaml = config.generate_github_actions();
        assert!(yaml.contains("DATABASE_URL"));
    }
}
