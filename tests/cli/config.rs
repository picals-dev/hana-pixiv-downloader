use std::fs;

use hana_pixiv_downloader::auth::Credential;
use wiremock::MockServer;

use crate::support::{cli::CliTestContext, config::toml_path_value};

#[tokio::test]
async fn config_show_prints_current_values_table() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    write_sample_config(&ctx);

    let output = ctx
        .command()
        .args(["config", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);

    assert!(stdout.contains("配置字段"));
    assert!(stdout.contains("当前值"));
    assert!(stdout.contains("auth.phpsessid"));
    assert!(stdout.contains("cookie-value"));
    assert!(stdout.contains("download.batch_layout"));
    assert!(stdout.contains("flat"));
    assert!(stdout.contains("download.sort"));
    assert!(stdout.contains("date_asc"));
    assert!(stdout.contains("proxy.url"));
    assert!(stdout.contains("socks5://127.0.0.1:1080"));
}

#[tokio::test]
async fn config_set_help_reuses_same_table_as_config_show() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    write_sample_config(&ctx);

    let show_output = ctx
        .command()
        .args(["config", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help_output = ctx
        .command()
        .args(["config", "set", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let show_stdout = String::from_utf8_lossy(&show_output);
    let help_stdout = String::from_utf8_lossy(&help_output);

    assert!(help_stdout.contains("hpd config set <KEY> <VALUE>"));
    assert_eq!(extract_table(&show_stdout), extract_table(&help_stdout));
}

#[tokio::test]
async fn config_set_without_args_shows_reference_table_in_stderr() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    write_sample_config(&ctx);

    let help_output = ctx
        .command()
        .args(["config", "set", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let missing_output = ctx
        .command()
        .args(["config", "set"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let help_stdout = String::from_utf8_lossy(&help_output);
    let missing_stderr = String::from_utf8_lossy(&missing_output);

    assert!(missing_stderr.contains("请提供配置键和值"));
    assert!(missing_stderr.contains("hpd config set <KEY> <VALUE>"));
    assert_eq!(extract_table(&help_stdout), extract_table(&missing_stderr));
}

#[tokio::test]
async fn config_show_accepts_windows_style_paths_in_config_file() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let roots = [
        r"C:\Users\runneradmin\Downloads\Pixiv\illust",
        r"C:\Users\runneradmin\Downloads\Pixiv\user",
        r"C:\Users\runneradmin\Downloads\Pixiv\bookmark",
        r"C:\Users\runneradmin\Downloads\Pixiv\keyword",
        r"C:\Users\runneradmin\Downloads\Pixiv\ranking",
    ];
    write_sample_config_with_roots(&ctx, &roots);

    let output = ctx
        .command()
        .args(["config", "show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);

    assert!(stdout.contains(r"C:\Users\runneradmin\Downloads\Pixiv\illust"));
    assert!(stdout.contains(r"C:\Users\runneradmin\Downloads\Pixiv\ranking"));
}

fn write_sample_config(ctx: &CliTestContext) {
    let roots = [
        ctx.path("downloads/illust").display().to_string(),
        ctx.path("downloads/user").display().to_string(),
        ctx.path("downloads/bookmark").display().to_string(),
        ctx.path("downloads/keyword").display().to_string(),
        ctx.path("downloads/ranking").display().to_string(),
    ];
    write_sample_config_with_roots(ctx, &roots);
}

fn write_sample_config_with_roots(ctx: &CliTestContext, roots: &[impl AsRef<str>; 5]) {
    let config_dir = ctx.xdg_config_home().join("hana-pixiv-downloader");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!(
            r#"[download]
batch_layout = "flat"
count = 12
sort = "date_asc"
r18 = true
ai = false
concurrent = 3
timeout = 45
retry = 9
with_tags = true

[download.roots]
illust = {0}
user = {1}
bookmark = {2}
keyword = {3}
ranking = {4}

[proxy]
url = "socks5://127.0.0.1:1080"
"#,
            toml_path_value(std::path::Path::new(roots[0].as_ref())),
            toml_path_value(std::path::Path::new(roots[1].as_ref())),
            toml_path_value(std::path::Path::new(roots[2].as_ref())),
            toml_path_value(std::path::Path::new(roots[3].as_ref())),
            toml_path_value(std::path::Path::new(roots[4].as_ref())),
        ),
    )
    .unwrap();

    ctx.write_credential(Credential::new_with_user_id("cookie-value", Some("12345678")).unwrap());
}

fn extract_table(output: &str) -> String {
    let mut lines = Vec::new();
    let mut started = false;

    for line in output.lines() {
        let is_table_line = matches!(line.chars().next(), Some('┌' | '├' | '│' | '└'));
        if !started {
            if is_table_line {
                started = true;
                lines.push(line);
            }
            continue;
        }

        if is_table_line {
            lines.push(line);
            continue;
        }

        if !line.trim().is_empty() {
            break;
        }

        break;
    }

    lines.join("\n")
}
