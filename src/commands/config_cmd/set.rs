use crate::{
    cli::config::SetConfigArgs,
    config::config_dir,
    error::{AppResult, CrawlerError},
};

use super::{
    field::{ConfigFieldKey, ConfigUpdateNote, parse_config_field_key},
    prompt::{ConfigPrompter, InteractiveConfigPrompter},
    render::render_set_help_output,
    shared::{CONFIG_SET_USAGE, ConfigSnapshot, load_config_snapshot},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedSetConfigArgs {
    key: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SetInvocation {
    Help,
    MissingArgs,
    Prompt(String),
    Update(ResolvedSetConfigArgs),
}

pub(crate) async fn set(args: SetConfigArgs) -> AppResult<()> {
    match resolve_set_invocation(args) {
        SetInvocation::Help => {
            let snapshot = load_config_snapshot()?;
            println!("{}", render_set_help_output(&snapshot, &config_dir()?));
            Ok(())
        }
        SetInvocation::MissingArgs => {
            let snapshot = load_config_snapshot()?;
            eprintln!("{}", render_set_help_output(&snapshot, &config_dir()?));
            Err(
                CrawlerError::InvalidInput(format!("请提供配置键和值。用法: {CONFIG_SET_USAGE}"))
                    .into(),
            )
        }
        SetInvocation::Prompt(key) => prompt_and_apply_config_update(key),
        SetInvocation::Update(args) => apply_config_update(args),
    }
}

fn prompt_and_apply_config_update(key: String) -> AppResult<()> {
    let snapshot = load_config_snapshot()?;
    let field = match parse_config_field_key(&key) {
        Ok(field) => field,
        Err(error) => {
            eprintln!("{}", render_set_help_output(&snapshot, &config_dir()?));
            return Err(error);
        }
    };

    let args = resolve_prompted_update_for_field(field, &snapshot, &InteractiveConfigPrompter)?;
    apply_config_update(args)
}

fn apply_config_update(args: ResolvedSetConfigArgs) -> AppResult<()> {
    let updated_key = args.key.clone();
    let field = parse_config_field_key(&args.key)?;
    let note = field.apply_value(&args.value)?;

    if note == ConfigUpdateNote::BatchLayoutChanged {
        println!("提示：该设置只会影响后续批量下载。");
        println!("如需按新布局整理之前设定下的已有目录，请运行：");
        println!("  hpd organize --dry-run");
        println!("  hpd organize --yes");
    }

    println!("✅ 已更新配置：{updated_key}");
    Ok(())
}

fn resolve_set_invocation(args: SetConfigArgs) -> SetInvocation {
    if args.help {
        return SetInvocation::Help;
    }

    match (args.key, args.value) {
        (Some(key), Some(value)) => SetInvocation::Update(ResolvedSetConfigArgs { key, value }),
        (Some(key), None) => SetInvocation::Prompt(key),
        _ => SetInvocation::MissingArgs,
    }
}

fn resolve_prompted_update_for_field(
    field: ConfigFieldKey,
    snapshot: &ConfigSnapshot,
    prompter: &impl ConfigPrompter,
) -> AppResult<ResolvedSetConfigArgs> {
    Ok(ResolvedSetConfigArgs {
        key: field.key().to_string(),
        value: field.prompt_value(snapshot, prompter)?,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{
        auth::Credential,
        cli::config::SetConfigArgs,
        test_support::{EnvVarGuard, lock_env},
    };

    use super::{SetInvocation, resolve_set_invocation, set};

    #[tokio::test]
    async fn config_set_can_update_auth_keys() {
        let _lock = lock_env().await;
        let temp = tempdir().unwrap();
        let xdg_home = temp.path().join(".config");
        let _xdg = EnvVarGuard::set("XDG_CONFIG_HOME", &xdg_home);

        set(SetConfigArgs {
            help: false,
            key: Some("auth.phpsessid".to_string()),
            value: Some("cookie-value".to_string()),
        })
        .await
        .unwrap();
        set(SetConfigArgs {
            help: false,
            key: Some("auth.user_id".to_string()),
            value: Some("12345678".to_string()),
        })
        .await
        .unwrap();

        let credential = Credential::load().unwrap().unwrap();
        assert_eq!(credential.phpsessid, "cookie-value");
        assert_eq!(credential.user_id(), Some("12345678"));
    }

    #[test]
    fn config_set_key_only_enters_prompt_mode() {
        let invocation = resolve_set_invocation(SetConfigArgs {
            help: false,
            key: Some("download.batch_layout".to_string()),
            value: None,
        });

        assert_eq!(
            invocation,
            SetInvocation::Prompt("download.batch_layout".to_string())
        );
    }
}
