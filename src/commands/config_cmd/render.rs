use std::path::Path;

use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};

use super::{
    field::CONFIG_FIELDS,
    shared::{CONFIG_SET_USAGE, ConfigSnapshot},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigTableRow {
    key: &'static str,
    value: String,
}

pub(in crate::commands::config_cmd) fn render_show_output(
    snapshot: &ConfigSnapshot,
    config_dir: &Path,
) -> String {
    format!(
        "配置目录: {}\n\n{}",
        config_dir.display(),
        render_config_table(snapshot)
    )
}

pub(in crate::commands::config_cmd) fn render_set_help_output(
    snapshot: &ConfigSnapshot,
    config_dir: &Path,
) -> String {
    format!(
        "用法:\n  {CONFIG_SET_USAGE}\n\n配置目录: {}\n\n可设置的配置字段与当前值：\n{}",
        config_dir.display(),
        render_config_table(snapshot)
    )
}

pub(in crate::commands::config_cmd) fn render_config_table(snapshot: &ConfigSnapshot) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["配置字段", "当前值"]);

    for row in collect_config_rows(snapshot) {
        table.add_row(vec![row.key, row.value.as_str()]);
    }

    table.to_string()
}

fn collect_config_rows(snapshot: &ConfigSnapshot) -> Vec<ConfigTableRow> {
    CONFIG_FIELDS
        .into_iter()
        .map(|field| ConfigTableRow {
            key: field.key(),
            value: field.current_value(snapshot),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        auth::Credential,
        config::{Config, DownloadRootsConfig, SortOrder},
    };

    use super::{
        super::shared::ConfigSnapshot, render_config_table, render_set_help_output,
        render_show_output,
    };

    #[test]
    fn config_show_and_set_help_reuse_same_table() {
        let mut config = Config::default();
        config.download.count = 12;
        config.download.sort = SortOrder::DateAsc;
        config.download.roots = DownloadRootsConfig {
            illust: "/tmp/illust".to_string(),
            user: "/tmp/user".to_string(),
            bookmark: "/tmp/bookmark".to_string(),
            keyword: "/tmp/keyword".to_string(),
            ranking: "/tmp/ranking".to_string(),
        };
        config.proxy.url = "socks5://127.0.0.1:1080".to_string();

        let snapshot = ConfigSnapshot {
            config,
            credential: Some(
                Credential::new_with_user_id("cookie-value", Some("12345678")).unwrap(),
            ),
        };

        let table = render_config_table(&snapshot);
        let show_output = render_show_output(&snapshot, Path::new("/tmp/hpd"));
        let help_output = render_set_help_output(&snapshot, Path::new("/tmp/hpd"));

        assert!(table.contains("配置字段"));
        assert!(table.contains("当前值"));
        assert!(table.contains("auth.phpsessid"));
        assert!(table.contains("cookie-value"));
        assert!(table.contains("download.sort"));
        assert!(table.contains("date_asc"));
        assert!(table.contains("download.roots.ranking"));
        assert!(table.contains("/tmp/ranking"));
        assert!(table.contains("proxy.url"));
        assert!(table.contains("socks5://127.0.0.1:1080"));

        assert!(show_output.contains(&table));
        assert!(help_output.contains(&table));
        assert!(help_output.contains("hpd config set <KEY> <VALUE>"));
    }

    #[test]
    fn config_table_marks_missing_values_as_unset() {
        let snapshot = ConfigSnapshot {
            config: Config::default(),
            credential: None,
        };

        let table = render_config_table(&snapshot);

        assert!(table.contains("auth.phpsessid"));
        assert!(table.contains("<未设置>"));
        assert!(table.contains("proxy.url"));
    }
}
