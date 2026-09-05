use anyhow::{bail, Context, Result};
use base64::Engine;
use clap::{Args, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::util::project_root;

// ========================================
// Virtual File System for In-Memory Deployment
// ========================================

/// Represents a file in memory with its deployment path and content
#[derive(Clone)]
struct VirtualFile {
    path: String,      // Deployment path (relative, no leading slash)
    content: Vec<u8>,  // File content in bytes
    hash: String,      // MD5 hash (computed on creation)
    mime_type: String, // MIME type for Content-Type header
}

impl VirtualFile {
    fn new(path: String, content: Vec<u8>, mime_type: String) -> Self {
        let hash = format!("{:x}", md5::compute(&content));
        Self {
            path,
            content,
            hash,
            mime_type,
        }
    }
}

/// Collection of virtual files ready for deployment
struct VirtualSite {
    files: Vec<VirtualFile>,
}

impl VirtualSite {
    fn new() -> Self {
        Self { files: Vec::new() }
    }

    fn add_file(&mut self, path: String, content: Vec<u8>, mime_type: String) {
        self.files.push(VirtualFile::new(path, content, mime_type));
    }

    fn get_file(&self, path: &str) -> Option<&VirtualFile> {
        self.files.iter().find(|f| f.path == path)
    }

    fn iter(&self) -> impl Iterator<Item = &VirtualFile> {
        self.files.iter()
    }

    fn len(&self) -> usize {
        self.files.len()
    }
}

#[derive(Args)]
pub struct WasmCmd {
    #[command(subcommand)]
    pub command: WasmSubCmd,
}

#[derive(Subcommand)]
pub enum WasmSubCmd {
    /// Build the WASM package only
    Build {
        /// Build in release mode (optimized)
        #[arg(long, short)]
        release: bool,
    },
    /// Build and serve locally for development
    Serve {
        /// Port to serve on
        #[arg(long, short, default_value = "8080")]
        port: u16,
        /// Build in release mode (optimized)
        #[arg(long, short)]
        release: bool,
        /// Automatically open browser
        #[arg(long)]
        open: bool,
    },
    /// Package WASM site for deployment
    Package {
        /// Output directory for packaged files
        #[arg(long, default_value = "target/wasm-site")]
        output: String,
        /// Build in release mode (optimized)
        #[arg(long, short)]
        release: bool,
    },
    /// Deploy WASM site to Cloudflare Pages
    Deploy {
        /// Branch name for deployment (auto-detected from git if not specified)
        /// Production = main/master, others = preview
        #[arg(long, short)]
        branch: Option<String>,
        /// Cloudflare Pages project name
        #[arg(long, default_value = "razemify-wasm")]
        project_name: String,
        /// Output directory to deploy
        #[arg(long, default_value = "target/wasm-site")]
        output: String,
        /// Skip packaging step (deploy pre-packaged files)
        #[arg(long)]
        skip_package: bool,
        /// Build in release mode (only if packaging)
        #[arg(long, short)]
        release: bool,
    },
    /// Clean up old deployments
    Cleanup {
        /// Cloudflare Pages project name
        #[arg(long, default_value = "razemify-wasm")]
        project_name: String,
        /// Keep this many recent deployments per environment (production/preview)
        #[arg(long, default_value = "5")]
        keep: usize,
        /// Actually delete (without this flag, only shows what would be deleted)
        #[arg(long)]
        yes: bool,
    },
    /// Promote a preview deployment to production
    Promote {
        /// Deployment ID to promote (or use --latest)
        deployment_id: Option<String>,
        /// Promote the latest deployment from current branch
        #[arg(long)]
        latest: bool,
        /// Cloudflare Pages project name
        #[arg(long, default_value = "razemify-wasm")]
        project_name: String,
        /// Production branch name
        #[arg(long, default_value = "main")]
        production_branch: String,
    },
    /// Rollback production to a previous deployment
    Rollback {
        /// Deployment ID to rollback to (shows list if not provided)
        deployment_id: Option<String>,
        /// Cloudflare Pages project name
        #[arg(long, default_value = "razemify-wasm")]
        project_name: String,
        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,
    },
}

impl WasmCmd {
    pub fn run(self) -> Result<()> {
        match self.command {
            WasmSubCmd::Build { release } => build_wasm(release),
            WasmSubCmd::Serve {
                port,
                release,
                open,
            } => serve_wasm(port, release, open),
            WasmSubCmd::Package { output, release } => package_wasm(&output, release),
            WasmSubCmd::Deploy {
                branch,
                project_name,
                output,
                skip_package,
                release,
            } => deploy_wasm(branch, &project_name, &output, skip_package, release),
            WasmSubCmd::Cleanup {
                project_name,
                keep,
                yes,
            } => cleanup_deployments(&project_name, keep, yes),
            WasmSubCmd::Promote {
                deployment_id,
                latest,
                project_name,
                production_branch,
            } => promote_deployment(deployment_id, latest, &project_name, &production_branch),
            WasmSubCmd::Rollback {
                deployment_id,
                project_name,
                yes,
            } => rollback_deployment(deployment_id, &project_name, yes),
        }
    }
}

fn build_wasm(release: bool) -> Result<()> {
    let project_root = project_root();
    let wasm_dir = project_root.join("razemify-wasm");

    if !wasm_dir.exists() {
        bail!(
            "razemify-wasm directory not found at {}",
            wasm_dir.display()
        );
    }

    println!("Building WASM package...");
    let mode = if release { "opt" } else { "dbg" };
    println!("Mode: {}", if release { "release" } else { "dev" });

    let status = Command::new("bazel")
        .args([
            "build",
            "--remote_download_toplevel",
            "-c",
            mode,
            "//razemify-wasm:razemify_wasm",
        ])
        .current_dir(&project_root)
        .status()
        .context("Failed to run Bazel. Install Bazelisk or Bazel and ensure it is on PATH")?;

    if !status.success() {
        bail!("Bazel WASM build failed");
    }

    let pkg_dir = wasm_dir.join("pkg");
    let bazel_info = Command::new("bazel")
        .args(["info", "bazel-bin", "-c", mode])
        .current_dir(&project_root)
        .output()
        .context("Failed to locate Bazel output directory")?;
    if !bazel_info.status.success() {
        bail!("bazel info bazel-bin failed");
    }

    let bazel_bin = String::from_utf8(bazel_info.stdout)
        .context("Bazel output directory is not valid UTF-8")?;
    let generated_pkg = Path::new(bazel_bin.trim())
        .join("razemify-wasm")
        .join("razemify_wasm");

    if pkg_dir.exists() {
        fs::remove_dir_all(&pkg_dir).context(format!("Failed to clean {}", pkg_dir.display()))?;
    }
    copy_directory(&generated_pkg, &pkg_dir)?;
    println!("✓ WASM build complete: {}", pkg_dir.display());

    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .context(format!("Failed to create {}", destination.display()))?;

    for entry in
        fs::read_dir(source).context(format!("Failed to read directory: {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).context(format!(
                "Failed to copy {} to {}",
                source_path.display(),
                destination_path.display()
            ))?;
        }
    }

    Ok(())
}

// ========================================
// Virtual Site Builder
// ========================================

fn build_virtual_site(release: bool) -> Result<VirtualSite> {
    let project_root = project_root();
    let wasm_dir = project_root.join("razemify-wasm");
    let pkg_dir = wasm_dir.join("pkg");
    let www_dir = wasm_dir.join("www");

    // Ensure WASM is built
    if !pkg_dir.exists() {
        println!("WASM package not found, building first...");
        build_wasm(release)?;
    }

    let mut site = VirtualSite::new();
    println!("Building virtual site...");

    // Load all files into memory
    load_index_html(&www_dir, &mut site)?;
    load_and_rewrite_index_js(&www_dir, &mut site)?;
    load_headers_file(&www_dir, &mut site)?;
    load_models_directory(&www_dir, &mut site)?;
    load_pkg_directory(&pkg_dir, &mut site)?;

    // Validate critical files
    let required = ["index.html", "index.js", "pkg/razemify_wasm_bg.wasm"];
    for file in required {
        if site.get_file(file).is_none() {
            bail!("Required file missing: {}", file);
        }
    }

    println!("✓ Virtual site ready with {} files", site.len());
    Ok(site)
}

fn load_index_html(www_dir: &Path, site: &mut VirtualSite) -> Result<()> {
    let src_html = www_dir.join("index.html");
    if src_html.exists() {
        let content =
            fs::read(&src_html).context(format!("Failed to read {}", src_html.display()))?;
        site.add_file("index.html".to_string(), content, "text/html".to_string());
        println!("  ✓ Loaded index.html");
    }
    Ok(())
}

fn load_and_rewrite_index_js(www_dir: &Path, site: &mut VirtualSite) -> Result<()> {
    let src = www_dir.join("index.js");
    if src.exists() {
        let content =
            fs::read_to_string(&src).context(format!("Failed to read {}", src.display()))?;

        // Rewrite import paths: '../pkg/' -> './pkg/'
        let rewritten = content.replace("'../pkg/", "'./pkg/");

        site.add_file(
            "index.js".to_string(),
            rewritten.into_bytes(),
            "application/javascript".to_string(),
        );
        println!("  ✓ Loaded and rewritten index.js");
    }
    Ok(())
}

fn load_headers_file(www_dir: &Path, site: &mut VirtualSite) -> Result<()> {
    let src_headers = www_dir.join("_headers");
    if src_headers.exists() {
        let content =
            fs::read(&src_headers).context(format!("Failed to read {}", src_headers.display()))?;
        site.add_file("_headers".to_string(), content, "text/plain".to_string());
        println!("  ✓ Loaded _headers");
    }
    Ok(())
}

fn load_models_directory(www_dir: &Path, site: &mut VirtualSite) -> Result<()> {
    let src_models = www_dir.join("models");
    if src_models.exists() {
        load_directory_recursive(&src_models, &src_models, "models", site)?;
        println!("  ✓ Loaded models/");
    }
    Ok(())
}

fn load_pkg_directory(pkg_dir: &Path, site: &mut VirtualSite) -> Result<()> {
    load_directory_recursive(pkg_dir, pkg_dir, "pkg", site)?;
    println!("  ✓ Loaded pkg/");
    Ok(())
}

fn load_directory_recursive(
    dir: &Path,
    base: &Path,
    prefix: &str,
    site: &mut VirtualSite,
) -> Result<()> {
    for entry in
        fs::read_dir(dir).context(format!("Failed to read directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            load_directory_recursive(&path, base, prefix, site)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(base)
                .context("Failed to get relative path")?
                .to_string_lossy()
                .replace('\\', "/");

            let deployment_path = format!("{}/{}", prefix, relative);
            let content = fs::read(&path).context(format!("Failed to read {}", path.display()))?;
            let mime_type = get_mime_type(&path).to_string();

            site.add_file(deployment_path, content, mime_type);
        }
    }
    Ok(())
}

fn write_virtual_site_to_disk(site: &VirtualSite, output_dir: &Path) -> Result<()> {
    // Clean and recreate directory
    if output_dir.exists() {
        fs::remove_dir_all(output_dir).context(format!(
            "Failed to clean directory: {}",
            output_dir.display()
        ))?;
    }
    fs::create_dir_all(output_dir).context(format!(
        "Failed to create directory: {}",
        output_dir.display()
    ))?;

    // Write each virtual file
    for file in site.iter() {
        let full_path = output_dir.join(&file.path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, &file.content)
            .context(format!("Failed to write {}", full_path.display()))?;
    }
    Ok(())
}

fn load_site_from_directory(dir: &Path) -> Result<VirtualSite> {
    let mut site = VirtualSite::new();

    fn visit_dir(dir: &Path, base: &Path, site: &mut VirtualSite) -> Result<()> {
        for entry in
            fs::read_dir(dir).context(format!("Failed to read directory: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                visit_dir(&path, base, site)?;
            } else if path.is_file() {
                let relative = path
                    .strip_prefix(base)
                    .context("Failed to get relative path")?
                    .to_string_lossy()
                    .replace('\\', "/");

                let content =
                    fs::read(&path).context(format!("Failed to read {}", path.display()))?;
                let mime_type = get_mime_type(&path).to_string();

                site.add_file(relative, content, mime_type);
            }
        }
        Ok(())
    }

    visit_dir(dir, dir, &mut site)?;
    Ok(site)
}

fn build_file_map(site: &VirtualSite) -> HashMap<String, (Vec<u8>, String)> {
    site.iter()
        .map(|vf| (vf.path.clone(), (vf.content.clone(), vf.hash.clone())))
        .collect()
}

fn serve_wasm(port: u16, release: bool, open: bool) -> Result<()> {
    // Validate port
    if port < 1024 {
        bail!("Port must be >= 1024 (non-privileged). Got: {}", port);
    }

    let project_root = project_root();
    let output_dir = project_root.join("target").join("wasm-site");

    // Build virtual site
    let site = build_virtual_site(release)?;

    // Write to disk for serving
    write_virtual_site_to_disk(&site, &output_dir)?;

    // Start server
    let addr = format!("127.0.0.1:{}", port);
    let server = tiny_http::Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to start server on {}: {}", addr, e))?;

    let url = format!("http://localhost:{}", port);

    println!("\n{}", "=".repeat(60));
    println!("WASM Development Server");
    println!("{}", "=".repeat(60));
    println!("  URL:       {}", url);
    println!("  Port:      {}", port);
    println!("  Directory: {}", output_dir.display());
    println!("  Mode:      {}", if release { "release" } else { "dev" });
    println!("{}", "=".repeat(60));
    println!("\nPress Ctrl+C to stop the server\n");

    if open {
        if let Err(e) = open_browser(&url) {
            println!("Failed to open browser: {}", e);
            println!("Please open {} manually", url);
        }
    }

    serve_static_files(&server, &output_dir)?;

    Ok(())
}

fn resolve_requested_file(
    url_path: &str,
    root: &Path,
    canonical_root: &Path,
) -> Option<std::path::PathBuf> {
    let file_path = if url_path == "/" || url_path.is_empty() {
        root.join("index.html")
    } else {
        root.join(url_path.trim_start_matches('/'))
    };

    let canonical = file_path.canonicalize().ok()?;
    if canonical.starts_with(canonical_root) && canonical.is_file() {
        Some(canonical)
    } else {
        None
    }
}

fn make_file_response(path: &Path, url_path: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error reading file {}: {}", path.display(), e);
            return tiny_http::Response::from_string("500 Internal Server Error").with_status_code(500);
        }
    };

    let mime_type = get_mime_type(path);
    let now = SystemTime::now();
    println!("[{}] {} -> {}", format_time(now), url_path, mime_type);

    let mut response = tiny_http::Response::from_data(data);
    let headers = [
        ("Cross-Origin-Embedder-Policy", "require-corp"),
        ("Cross-Origin-Opener-Policy", "same-origin"),
        ("Content-Type", mime_type),
    ];
    for (name, val) in headers {
        if let Ok(header) = tiny_http::Header::from_bytes(name.as_bytes(), val.as_bytes()) {
            response.add_header(header);
        }
    }
    response
}

fn serve_static_files(server: &tiny_http::Server, root: &Path) -> Result<()> {
    let canonical_root = root
        .canonicalize()
        .context("Failed to canonicalize root directory")?;

    loop {
        let request = match server.recv() {
            Ok(rq) => rq,
            Err(e) => {
                eprintln!("Error receiving request: {}", e);
                continue;
            }
        };

        let url_path = request.url();
        let response = match resolve_requested_file(url_path, root, &canonical_root) {
            Some(file) => make_file_response(&file, url_path),
            None => tiny_http::Response::from_string("404 Not Found").with_status_code(404),
        };

        if let Err(e) = request.respond(response) {
            eprintln!("Error sending response: {}", e);
        }
    }
}

fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()) {
        Some("html") => "text/html",
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("css") => "text/css",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("onnx") => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

fn format_time(time: SystemTime) -> String {
    let Ok(duration) = time.duration_since(UNIX_EPOCH) else {
        return "??:??:??".to_string();
    };
    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }

    Ok(())
}

fn cleanup_deployments(project_name: &str, keep: usize, yes: bool) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("Deployment Cleanup");
    println!("{}", "=".repeat(60));
    println!("  Project:      {}", project_name);
    println!("  Keep:         {} most recent per environment", keep);
    println!("  Mode:         {}", if yes { "DELETE" } else { "DRY RUN" });
    println!("{}", "=".repeat(60));
    println!();

    // Get API credentials
    let api_token = check_cloudflare_credentials()?;

    // Create async runtime
    let runtime = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

    runtime.block_on(async {
        // Get account ID
        let account_id = get_account_id(&api_token).await?;

        // Fetch all deployments
        println!("Fetching deployments...");
        let client = reqwest::Client::new();
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments",
            account_id, project_name
        );

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_token))
            .send()
            .await
            .context("Failed to fetch deployments")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Failed to fetch deployments (status: {}): {}", status, body);
        }

        let deployments: DeploymentsListResponse = response
            .json()
            .await
            .context("Failed to parse deployments")?;

        println!("✓ Found {} total deployments\n", deployments.result.len());

        // Group by environment
        let mut production: Vec<DeploymentInfo> = Vec::new();
        let mut preview: Vec<DeploymentInfo> = Vec::new();

        for deployment in deployments.result {
            match deployment.environment.as_str() {
                "production" => production.push(deployment),
                "preview" => preview.push(deployment),
                _ => {}
            }
        }

        // Sort by creation date (newest first)
        production.sort_by(|a, b| b.created_on.cmp(&a.created_on));
        preview.sort_by(|a, b| b.created_on.cmp(&a.created_on));

        println!("Environment breakdown:");
        println!("  Production:   {} deployments", production.len());
        println!("  Preview:      {} deployments", preview.len());
        println!();

        // Determine what to delete
        let prod_to_delete: Vec<_> = production.iter().skip(keep).collect();
        let preview_to_delete: Vec<_> = preview.iter().skip(keep).collect();

        let total_to_delete = prod_to_delete.len() + preview_to_delete.len();

        if total_to_delete == 0 {
            println!("✓ No deployments to clean up");
            return Ok(());
        }

        println!("Deployments to delete ({}):", total_to_delete);
        println!();

        if !prod_to_delete.is_empty() {
            println!("Production ({} to delete):", prod_to_delete.len());
            for dep in &prod_to_delete {
                let alias = dep
                    .aliases
                    .as_ref()
                    .and_then(|a| a.first())
                    .map(|s| s.as_str())
                    .unwrap_or("no alias");
                println!("  - {} | {} | {}", dep.short_id, dep.created_on, alias);
            }
            println!();
        }

        if !preview_to_delete.is_empty() {
            println!("Preview ({} to delete):", preview_to_delete.len());
            for dep in &preview_to_delete {
                let alias = dep
                    .aliases
                    .as_ref()
                    .and_then(|a| a.first())
                    .map(|s| s.as_str())
                    .unwrap_or("no alias");
                println!("  - {} | {} | {}", dep.short_id, dep.created_on, alias);
            }
            println!();
        }

        if !yes {
            println!("ℹ️  This is a DRY RUN. Use --yes to actually delete these deployments.");
            return Ok(());
        }

        // Actually delete
        println!("Deleting deployments...");
        let pb = indicatif::ProgressBar::new(total_to_delete as u64);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .expect("Progress bar template is valid")
                .progress_chars("=>-"),
        );

        let mut deleted = 0;
        let mut failed = 0;

        for dep in prod_to_delete.iter().chain(preview_to_delete.iter()) {
            pb.set_message(dep.short_id.to_string());

            let delete_url = format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments/{}",
                account_id, project_name, dep.id
            );

            let result = client
                .delete(&delete_url)
                .header("Authorization", format!("Bearer {}", api_token))
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => deleted += 1,
                _ => failed += 1,
            }

            pb.inc(1);
        }

        pb.finish_with_message("Complete");

        println!();
        println!("✓ Deleted: {}", deleted);
        if failed > 0 {
            println!("✗ Failed:  {}", failed);
        }

        Ok(())
    })
}

fn package_wasm(output: &str, release: bool) -> Result<()> {
    let project_root = project_root();
    let output_dir = project_root.join(output);

    // Build virtual site
    let site = build_virtual_site(release)?;

    // Write to disk (users expect physical files from package)
    write_virtual_site_to_disk(&site, &output_dir)?;

    println!("\n{}", "=".repeat(60));
    println!("Package Ready");
    println!("{}", "=".repeat(60));
    println!("  Location: {}", output_dir.display());
    println!("  Mode:     {}", if release { "release" } else { "dev" });
    println!("{}", "=".repeat(60));
    println!("\nTo deploy to Cloudflare Pages:");
    println!(
        "  npx wrangler pages deploy {} --project-name razemify",
        output_dir.display()
    );
    println!();

    Ok(())
}

// ========================================
// Deployment Functions
// ========================================

// Wrangler-style API structures

#[derive(serde::Deserialize, Debug)]
struct UploadTokenResponse {
    result: UploadTokenResult,
    #[allow(dead_code)]
    success: bool,
}

#[derive(serde::Deserialize, Debug)]
struct UploadTokenResult {
    jwt: String,
}

#[derive(serde::Serialize)]
struct CheckMissingRequest {
    hashes: Vec<String>,
}

#[derive(serde::Serialize)]
struct UploadPayloadFile {
    key: String,
    value: String,
    metadata: FileMetadata,
    base64: bool,
}

#[derive(serde::Serialize)]
struct FileMetadata {
    #[serde(rename = "contentType")]
    content_type: String,
}

#[derive(serde::Serialize)]
struct UpsertHashesRequest {
    hashes: Vec<String>,
}

#[derive(serde::Deserialize, Debug)]
struct CreateDeploymentResponse {
    result: DeploymentResult,
    #[allow(dead_code)]
    success: bool,
}

#[derive(serde::Deserialize, Debug)]
struct DeploymentResult {
    id: String,
    #[allow(dead_code)]
    url: String,
}

#[derive(serde::Deserialize, Debug)]
struct DeploymentsListResponse {
    result: Vec<DeploymentInfo>,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct DeploymentInfo {
    id: String,
    short_id: String,
    environment: String,
    #[allow(dead_code)]
    url: String,
    created_on: String,
    #[serde(default)]
    aliases: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct AccountsResponse {
    result: Vec<Account>,
}

#[derive(serde::Deserialize)]
struct Account {
    id: String,
    name: String,
}

#[derive(serde::Deserialize, Debug)]
struct DeploymentDetailResponse {
    result: DeploymentDetail,
}

#[derive(serde::Deserialize, Debug)]
struct DeploymentDetail {
    #[allow(dead_code)]
    id: String,
    short_id: String,
    environment: String,
    url: String,
    created_on: String,
    #[serde(default)]
    deployment_trigger: DeploymentTrigger,
    #[serde(default)]
    latest_stage: LatestStage,
}

#[derive(serde::Deserialize, Debug, Default)]
struct DeploymentTrigger {
    #[serde(rename = "metadata")]
    metadata: TriggerMetadata,
}

#[derive(serde::Deserialize, Debug, Default)]
struct TriggerMetadata {
    #[serde(default)]
    branch: String,
    #[serde(default)]
    commit_hash: String,
    #[serde(default)]
    commit_message: String,
}

#[derive(serde::Deserialize, Debug, Default)]
struct LatestStage {
    #[allow(dead_code)]
    name: String,
    status: String,
}

#[derive(serde::Deserialize, Debug)]
struct RollbackResponse {
    result: RollbackResult,
}

#[derive(serde::Deserialize, Debug)]
struct RollbackResult {
    id: String,
    url: String,
}

fn check_cloudflare_credentials() -> Result<String> {
    let api_token = std::env::var("CLOUDFLARE_API_TOKEN").context(
        "CLOUDFLARE_API_TOKEN environment variable not set\n\n\
             Get your API token from: https://dash.cloudflare.com/profile/api-tokens\n\
             Create a token with 'Cloudflare Pages' permissions\n\n\
             Then set: export CLOUDFLARE_API_TOKEN=your_token",
    )?;

    println!("✓ Cloudflare API token found");
    Ok(api_token)
}

pub async fn get_account_id(api_token: &str) -> Result<String> {
    // Check if account ID is explicitly provided
    if let Ok(account_id) = std::env::var("CLOUDFLARE_ACCOUNT_ID") {
        println!("✓ Using account ID from environment: {}", account_id);
        return Ok(account_id);
    }

    // Otherwise, fetch it from the API
    println!("Fetching account ID from Cloudflare API...");

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.cloudflare.com/client/v4/accounts")
        .header("Authorization", format!("Bearer {}", api_token))
        .send()
        .await
        .context("Failed to fetch accounts from Cloudflare API")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Failed to fetch accounts (status: {})\n\
             Response: {}\n\n\
             Check that your API token has correct permissions",
            status,
            body
        );
    }

    let accounts: AccountsResponse = response
        .json()
        .await
        .context("Failed to parse accounts response")?;

    if accounts.result.is_empty() {
        bail!(
            "No Cloudflare accounts found\n\n\
             Ensure your API token has access to at least one account"
        );
    }

    if accounts.result.len() > 1 {
        println!("\nMultiple accounts found:");
        for (i, account) in accounts.result.iter().enumerate() {
            println!("  {}. {} ({})", i + 1, account.name, account.id);
        }
        println!(
            "\nUsing first account: {} ({})",
            accounts.result[0].name, accounts.result[0].id
        );
        println!("To use a different account, set: export CLOUDFLARE_ACCOUNT_ID=<account_id>");
    }

    let account_id = accounts.result[0].id.clone();
    println!("✓ Account ID: {}", account_id);

    Ok(account_id)
}

fn determine_branch(explicit_branch: Option<String>) -> Result<String> {
    if let Some(branch) = explicit_branch {
        println!("Using explicit branch: {}", branch);
        return Ok(branch);
    }

    // Auto-detect from git
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .context("Failed to get current git branch")?;

    if !output.status.success() {
        bail!(
            "Not in a git repository or unable to determine branch\n\n\
             Use --branch <name> to specify deployment branch explicitly"
        );
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if branch.is_empty() {
        bail!(
            "Not on a git branch (detached HEAD?)\n\n\
             Use --branch <name> to specify deployment branch explicitly"
        );
    }

    println!("Auto-detected branch: {}", branch);
    Ok(branch)
}

fn git_cmd_stdout(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_git_commit_hash() -> Option<String> {
    git_cmd_stdout(&["rev-parse", "HEAD"])
}

fn get_git_commit_message() -> Option<String> {
    git_cmd_stdout(&["log", "-1", "--pretty=%B"])
}

fn deploy_wasm(
    branch: Option<String>,
    project_name: &str,
    output: &str,
    skip_package: bool,
    release: bool,
) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("WASM Deployment");
    println!("{}", "=".repeat(60));

    // Pre-flight checks
    let api_token = check_cloudflare_credentials()?;

    // Determine deployment target
    let branch = determine_branch(branch)?;
    let is_production = branch == "main" || branch == "master";

    let project_root = project_root();
    let output_dir = project_root.join(output);

    // Build virtual site in memory
    let site = if !skip_package {
        println!("\nBuilding WASM site...");
        let site = build_virtual_site(release)?;
        println!("✓ Build complete");
        site
    } else {
        // Load from disk for --skip-package
        if !output_dir.exists() {
            bail!(
                "Output directory does not exist: {}\n\n\
                 Run without --skip-package to build first",
                output_dir.display()
            );
        }
        println!(
            "✓ Loading pre-packaged files from: {}",
            output_dir.display()
        );
        load_site_from_directory(&output_dir)?
    };

    // Execute deployment (async operations)
    let runtime = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

    runtime.block_on(async {
        // Fetch account ID (or use from env)
        let account_id = get_account_id(&api_token).await?;

        // Display deployment info
        println!("\n{}", "=".repeat(60));
        println!("Deploying to Cloudflare Pages");
        println!("{}", "=".repeat(60));
        println!("  Project:      {}", project_name);
        println!("  Branch:       {}", branch);
        println!(
            "  Type:         {}",
            if is_production {
                "Production"
            } else {
                "Preview"
            }
        );
        println!("  Source:       Virtual (in-memory)");
        println!(
            "  Mode:         {}",
            if release { "Release" } else { "Debug" }
        );
        println!("{}", "=".repeat(60));
        println!();

        // Execute deployment
        deploy_to_cloudflare(&site, project_name, &branch, &api_token, &account_id).await?;

        // Success message
        println!("\n{}", "=".repeat(60));
        println!("✓ Deployment Successful");
        println!("{}", "=".repeat(60));

        if is_production {
            println!("  Production URL: https://{}.pages.dev", project_name);
        } else {
            println!(
                "  Preview URL:    https://{}.{}.pages.dev",
                branch, project_name
            );
        }

        println!();

        Ok(())
    })
}

async fn deploy_to_cloudflare(
    site: &VirtualSite,
    project_name: &str,
    branch: &str,
    api_token: &str,
    account_id: &str,
) -> Result<()> {
    println!("Collecting and hashing files...");
    let files = build_file_map(site);
    println!("✓ Found {} files", files.len());

    let client = reqwest::Client::new();

    // Step 1: Get upload token (JWT)
    println!("\nGetting upload token...");
    let token_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/upload-token",
        account_id, project_name
    );

    let token_response = client
        .get(&token_url)
        .header("Authorization", format!("Bearer {}", api_token))
        .send()
        .await
        .context("Failed to get upload token")?;

    if !token_response.status().is_success() {
        let status = token_response.status();
        let body = token_response.text().await.unwrap_or_default();
        bail!(
            "Failed to get upload token (status: {})\n\
             Response: {}\n\n\
             Possible solutions:\n\
             - Verify project '{}' exists in Cloudflare Pages dashboard\n\
             - Check API token has 'Cloudflare Pages' permissions",
            status,
            body,
            project_name
        );
    }

    let token_data: UploadTokenResponse = token_response
        .json()
        .await
        .context("Failed to parse upload token response")?;
    let jwt = token_data.result.jwt;
    println!("✓ Upload token obtained");

    // Step 2: Check which file hashes are missing
    // Exclude special files (_headers, _redirects) - they go directly in deployment FormData
    println!("\nChecking which files need upload...");
    let hashes: Vec<String> = files
        .iter()
        .filter(|(path, _)| *path != "_headers" && *path != "_redirects")
        .map(|(_, (_, hash))| hash.clone())
        .collect();

    let check_missing_url =
        "https://api.cloudflare.com/client/v4/pages/assets/check-missing".to_string();

    let check_response = client
        .post(&check_missing_url)
        .header("Authorization", format!("Bearer {}", jwt))
        .header("Content-Type", "application/json")
        .json(&CheckMissingRequest {
            hashes: hashes.clone(),
        })
        .send()
        .await
        .context("Failed to check missing hashes")?;

    if !check_response.status().is_success() {
        let status = check_response.status();
        let body = check_response.text().await.unwrap_or_default();
        bail!(
            "Failed to check missing hashes (status: {}): {}",
            status,
            body
        );
    }

    let missing_hashes: Vec<String> = check_response
        .json::<serde_json::Value>()
        .await
        .context("Failed to parse check-missing response")?
        .get("result")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let skipped = files.len() - missing_hashes.len();
    if skipped > 0 {
        println!("✓ {} files already cached", skipped);
    }

    // Step 3: Upload missing files
    if !missing_hashes.is_empty() {
        println!("Uploading {} new files...", missing_hashes.len());

        let missing_set: std::collections::HashSet<String> =
            missing_hashes.iter().cloned().collect();

        // Separate small and large files (25 MB threshold)
        const MAX_BATCH_SIZE: usize = 25 * 1024 * 1024; // 25 MB
        let mut small_files = Vec::new();
        let mut large_files = Vec::new();

        for (path, (content, hash)) in &files {
            if missing_set.contains(hash) {
                let file_size = content.len();

                if file_size > MAX_BATCH_SIZE {
                    large_files.push((path, content, hash, file_size));
                } else {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(content);

                    // Get MIME type from virtual site
                    let mime_type = site
                        .get_file(path)
                        .map(|vf| vf.mime_type.clone())
                        .unwrap_or_else(|| "application/octet-stream".to_string());

                    small_files.push(UploadPayloadFile {
                        key: hash.clone(),
                        value: encoded,
                        metadata: FileMetadata {
                            content_type: mime_type,
                        },
                        base64: true,
                    });
                }
            }
        }

        // Upload small files in batch
        if !small_files.is_empty() {
            println!("  Uploading {} small files in batch...", small_files.len());
            let upload_url = "https://api.cloudflare.com/client/v4/pages/assets/upload".to_string();

            let upload_response = client
                .post(&upload_url)
                .header("Authorization", format!("Bearer {}", jwt))
                .header("Content-Type", "application/json")
                .json(&small_files)
                .send()
                .await
                .context("Failed to upload small files")?;

            if !upload_response.status().is_success() {
                let status = upload_response.status();
                let body = upload_response.text().await.unwrap_or_default();
                bail!(
                    "Failed to upload small files (status: {}): {}",
                    status,
                    body
                );
            }
            println!("  ✓ Small files uploaded");
        }

        // Upload large files individually using direct PUT
        if !large_files.is_empty() {
            println!(
                "  Uploading {} large files individually...",
                large_files.len()
            );

            for (i, (path, content, hash, size)) in large_files.iter().enumerate() {
                println!(
                    "    [{}/{}] {} ({:.1} MB)...",
                    i + 1,
                    large_files.len(),
                    path,
                    *size as f64 / 1_048_576.0
                );

                // Get MIME type from virtual site
                let mime_type = site
                    .get_file(path)
                    .map(|vf| vf.mime_type.as_str())
                    .unwrap_or("application/octet-stream");

                // Try uploading with binary body instead of base64 JSON
                let upload_url =
                    format!("https://api.cloudflare.com/client/v4/pages/assets/{}", hash);

                let upload_response = client
                    .put(&upload_url)
                    .header("Authorization", format!("Bearer {}", jwt))
                    .header("Content-Type", mime_type)
                    .body((*content).clone())
                    .send()
                    .await;

                // If PUT to /pages/assets/{hash} fails, fall back to JSON upload endpoint
                let result: Result<()> = match upload_response {
                    Ok(resp) if resp.status().is_success() => {
                        println!("      ✓ Uploaded via PUT");
                        Ok(())
                    }
                    _ => {
                        println!("      PUT failed, trying JSON upload...");
                        let encoded = base64::engine::general_purpose::STANDARD.encode(content);

                        let payload = vec![UploadPayloadFile {
                            key: (*hash).clone(),
                            value: encoded,
                            metadata: FileMetadata {
                                content_type: mime_type.to_string(),
                            },
                            base64: true,
                        }];

                        let upload_url =
                            "https://api.cloudflare.com/client/v4/pages/assets/upload".to_string();
                        let resp = client
                            .post(&upload_url)
                            .header("Authorization", format!("Bearer {}", jwt))
                            .header("Content-Type", "application/json")
                            .json(&payload)
                            .send()
                            .await
                            .context(format!("Failed to upload large file: {}", path))?;

                        if !resp.status().is_success() {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            bail!(
                                "Failed to upload large file {} (status: {}): {}",
                                path,
                                status,
                                body
                            );
                        }
                        println!("      ✓ Uploaded via JSON");
                        Ok(())
                    }
                };

                result?;
            }
            println!("  ✓ Large files uploaded");
        }

        println!("✓ All files uploaded successfully");
    } else {
        println!("✓ No new files to upload");
    }

    // Step 4: Upsert hashes
    println!("\nRegistering file hashes...");
    let upsert_url = "https://api.cloudflare.com/client/v4/pages/assets/upsert-hashes".to_string();

    let upsert_response = client
        .post(&upsert_url)
        .header("Authorization", format!("Bearer {}", jwt))
        .header("Content-Type", "application/json")
        .json(&UpsertHashesRequest { hashes })
        .send()
        .await
        .context("Failed to upsert hashes")?;

    if !upsert_response.status().is_success() {
        let status = upsert_response.status();
        let body = upsert_response.text().await.unwrap_or_default();
        bail!("Failed to upsert hashes (status: {}): {}", status, body);
    }

    println!("✓ File hashes registered");

    // Step 5: Create deployment with manifest
    println!("\nCreating deployment...");

    // Build manifest - wrangler uses leading slashes!
    // But exclude special files like _headers, _redirects which must be uploaded as File objects
    let mut manifest_map = HashMap::new();
    for (path, (_content, hash)) in &files {
        // Skip special files that need to be uploaded directly
        if path == "_headers" || path == "_redirects" {
            continue;
        }
        manifest_map.insert(format!("/{}", path), hash.clone());
    }

    let manifest_json =
        serde_json::to_string(&manifest_map).context("Failed to serialize manifest")?;

    println!("  Manifest has {} entries", manifest_map.len());
    if manifest_map.len() < 15 {
        println!("  Manifest: {}", manifest_json);
    }

    let commit_hash = get_git_commit_hash();
    let commit_message = get_git_commit_message();

    let deployment_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments",
        account_id, project_name
    );

    // Build FormData in same order as wrangler
    let mut form = reqwest::multipart::Form::new().text("manifest", manifest_json);

    if let Some(message) = commit_message {
        form = form.text("commit_message", message);
    }
    if let Some(hash) = commit_hash {
        form = form.text("commit_hash", hash);
    }

    // Always mark as dirty to match wrangler behavior
    form = form.text("commit_dirty", "true");

    // Branch after metadata
    form = form.text("branch", branch.to_string());

    // Add special files as File objects after text fields
    if let Some((content, _hash)) = files.get("_headers") {
        form = form.part(
            "_headers",
            reqwest::multipart::Part::bytes(content.clone()).file_name("_headers"),
        );
        println!("  ✓ Including _headers file");
    }

    if let Some((content, _hash)) = files.get("_redirects") {
        form = form.part(
            "_redirects",
            reqwest::multipart::Part::bytes(content.clone()).file_name("_redirects"),
        );
        println!("  ✓ Including _redirects file");
    }

    let deployment_response = client
        .post(&deployment_url)
        .header("Authorization", format!("Bearer {}", api_token))
        .multipart(form)
        .send()
        .await
        .context("Failed to create deployment")?;

    if !deployment_response.status().is_success() {
        let status = deployment_response.status();
        let body = deployment_response.text().await.unwrap_or_default();
        bail!(
            "Failed to create deployment (status: {})\n\
             Response: {}",
            status,
            body
        );
    }

    let deployment: CreateDeploymentResponse = deployment_response
        .json()
        .await
        .context("Failed to parse deployment response")?;

    println!("✓ Deployment created: {}", deployment.result.id);

    Ok(())
}

fn promote_deployment(
    deployment_id: Option<String>,
    latest: bool,
    project_name: &str,
    production_branch: &str,
) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("Promote Deployment to Production");
    println!("{}", "=".repeat(60));

    // Get credentials
    let api_token = check_cloudflare_credentials()?;

    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let account_id = get_account_id(&api_token).await?;
        let client = reqwest::Client::new();

        // Determine which deployment to promote
        let deployment_id = if latest {
            println!("Finding latest preview deployment from current branch...");
            let current_branch = determine_branch(None)?;

            // Fetch deployments
            let url = format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments",
                account_id, project_name
            );

            let response = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", api_token))
                .send()
                .await
                .context("Failed to fetch deployments")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                bail!("Failed to fetch deployments (status: {}): {}", status, body);
            }

            let deployments: DeploymentsListResponse = response
                .json()
                .await
                .context("Failed to parse deployments")?;

            // Find latest preview deployment from current branch
            let latest_deployment = deployments
                .result
                .iter()
                .filter(|d| d.environment == "preview")
                .find(|d| {
                    d.aliases
                        .as_ref()
                        .map(|aliases| {
                            aliases
                                .iter()
                                .any(|alias| alias.starts_with(&format!("{}.", current_branch)))
                        })
                        .unwrap_or(false)
                });

            match latest_deployment {
                Some(dep) => {
                    println!("✓ Found: {} ({})", dep.short_id, dep.created_on);
                    dep.id.clone()
                }
                None => {
                    bail!(
                        "No preview deployments found for branch '{}'\n\
                         Deploy to preview first: cargo xtask wasm deploy",
                        current_branch
                    );
                }
            }
        } else if let Some(id) = deployment_id {
            id
        } else {
            bail!("Either provide a deployment ID or use --latest flag");
        };

        // Fetch deployment details
        println!("\nFetching deployment details...");
        let detail_url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments/{}",
            account_id, project_name, deployment_id
        );

        let response = client
            .get(&detail_url)
            .header("Authorization", format!("Bearer {}", api_token))
            .send()
            .await
            .context("Failed to fetch deployment details")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!(
                "Failed to fetch deployment details (status: {}): {}\n\
                 Check that the deployment ID is correct",
                status,
                body
            );
        }

        let deployment: DeploymentDetailResponse = response
            .json()
            .await
            .context("Failed to parse deployment details")?;

        let dep = deployment.result;

        if dep.environment == "production" {
            bail!(
                "Deployment {} is already in production\n\
                 Use 'cargo xtask wasm rollback' to rollback production instead",
                dep.short_id
            );
        }

        println!("✓ Deployment Details:");
        println!("  ID:          {}", dep.short_id);
        println!("  Environment: {}", dep.environment);
        println!("  Branch:      {}", dep.deployment_trigger.metadata.branch);
        println!(
            "  Commit:      {}",
            dep.deployment_trigger.metadata.commit_hash
        );
        println!("  Status:      {}", dep.latest_stage.status);
        println!("  URL:         {}", dep.url);

        if dep.latest_stage.status != "success" {
            bail!(
                "Cannot promote deployment with status '{}'\n\
                 Only successful deployments can be promoted",
                dep.latest_stage.status
            );
        }

        println!("\n{}", "=".repeat(60));
        println!("Promoting to Production");
        println!("{}", "=".repeat(60));
        println!("  Target branch: {}", production_branch);
        println!(
            "  Commit:        {}",
            dep.deployment_trigger.metadata.commit_hash
        );
        println!("{}", "=".repeat(60));
        println!();

        Ok(())
    })?;

    // Use the existing deploy command to deploy to production
    println!("Deploying to production branch...");
    deploy_wasm(
        Some(production_branch.to_string()),
        project_name,
        "target/wasm-site",
        false, // don't skip package
        true,  // always use release mode for production
    )?;

    println!("\n{}", "=".repeat(60));
    println!("✓ Promotion Complete");
    println!("{}", "=".repeat(60));
    println!("  Production URL: https://{}.pages.dev", project_name);
    println!("{}", "=".repeat(60));
    println!();

    Ok(())
}

fn rollback_deployment(deployment_id: Option<String>, project_name: &str, yes: bool) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("Rollback Production Deployment");
    println!("{}", "=".repeat(60));

    let api_token = check_cloudflare_credentials()?;
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let account_id = get_account_id(&api_token).await?;
        let client = reqwest::Client::new();

        // If no deployment ID provided, show recent production deployments
        let deployment_id = if let Some(id) = deployment_id {
            id
        } else {
            println!("\nFetching recent production deployments...");
            let url = format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments",
                account_id, project_name
            );

            let response = client
                .get(&url)
                .header("Authorization", format!("Bearer {}", api_token))
                .send()
                .await
                .context("Failed to fetch deployments")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                bail!("Failed to fetch deployments (status: {}): {}", status, body);
            }

            let deployments: DeploymentsListResponse = response
                .json()
                .await
                .context("Failed to parse deployments")?;

            // Filter and sort production deployments
            let mut production: Vec<_> = deployments
                .result
                .into_iter()
                .filter(|d| d.environment == "production")
                .collect();
            production.sort_by(|a, b| b.created_on.cmp(&a.created_on));

            if production.is_empty() {
                bail!("No production deployments found");
            }

            println!("\nRecent production deployments:");
            println!("{}", "-".repeat(80));
            for (i, dep) in production.iter().take(10).enumerate() {
                let marker = if i == 0 { "→ CURRENT" } else { "" };
                let alias = dep
                    .aliases
                    .as_ref()
                    .and_then(|a| a.first())
                    .map(|s| s.as_str())
                    .unwrap_or("no alias");
                println!(
                    "  {}. {} | {} | {} {}",
                    i + 1,
                    dep.short_id,
                    dep.created_on,
                    alias,
                    marker
                );
            }
            println!("{}", "-".repeat(80));
            println!("\nTo rollback, run: cargo xtask wasm rollback <deployment_id>");
            return Ok(());
        };

        // Fetch deployment details to confirm
        println!("\nFetching deployment details...");
        let detail_url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments/{}",
            account_id, project_name, deployment_id
        );

        let response = client
            .get(&detail_url)
            .header("Authorization", format!("Bearer {}", api_token))
            .send()
            .await
            .context("Failed to fetch deployment details")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!(
                "Failed to fetch deployment details (status: {}): {}",
                status,
                body
            );
        }

        let deployment: DeploymentDetailResponse = response
            .json()
            .await
            .context("Failed to parse deployment details")?;

        let dep = deployment.result;

        if dep.environment != "production" {
            bail!(
                "Deployment {} is not a production deployment\n\
                 Rollback only works with production deployments",
                dep.short_id
            );
        }

        println!("✓ Rollback Target:");
        println!("  ID:      {}", dep.short_id);
        println!("  Created: {}", dep.created_on);
        println!("  Commit:  {}", dep.deployment_trigger.metadata.commit_hash);
        println!("  Message: {}", dep.deployment_trigger.metadata.commit_message);
        println!();

        if !yes {
            println!("⚠️  This will rollback production to deployment {}", dep.short_id);
            println!("   Run with --yes to confirm");
            return Ok(());
        }

        // Execute rollback
        println!("Rolling back production...");
        let rollback_url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects/{}/deployments/{}/rollback",
            account_id, project_name, deployment_id
        );

        let response = client
            .post(&rollback_url)
            .header("Authorization", format!("Bearer {}", api_token))
            .send()
            .await
            .context("Failed to rollback deployment")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!(
                "Failed to rollback deployment (status: {}): {}\n\
                 Note: You can only rollback to previous production deployments",
                status,
                body
            );
        }

        let rollback: RollbackResponse = response
            .json()
            .await
            .context("Failed to parse rollback response")?;

        println!("\n{}", "=".repeat(60));
        println!("✓ Rollback Complete");
        println!("{}", "=".repeat(60));
        println!("  Deployment ID: {}", rollback.result.id);
        println!("  URL:           {}", rollback.result.url);
        println!("{}", "=".repeat(60));
        println!();

        Ok(())
    })
}
