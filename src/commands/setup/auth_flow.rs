use url::Url;

use crate::{
    auth::Credential,
    config::{Config, DownloadMode, DownloadOverrides, EnvOverrides, ResolvedDownloadOptions},
    error::AppResult,
    net::PixivNetSession,
    pixiv::selector::select_current_user_id,
};

pub(crate) async fn fetch_current_user_id_from_pixiv(credential: &Credential) -> AppResult<String> {
    let config = Config::load()?;
    let env = EnvOverrides::from_process_env()?;
    let options = config.resolve_download_options(
        DownloadMode::Illust,
        &env,
        &DownloadOverrides::default(),
    )?;
    fetch_current_user_id_with_options(credential, &options).await
}

async fn fetch_current_user_id_with_options(
    credential: &Credential,
    options: &ResolvedDownloadOptions,
) -> AppResult<String> {
    let base_url = crate::net::resolve_base_url(None)?;
    fetch_current_user_id_with_base_url(credential, options, base_url).await
}

async fn fetch_current_user_id_with_base_url(
    credential: &Credential,
    options: &ResolvedDownloadOptions,
    base_url: Url,
) -> AppResult<String> {
    let session =
        PixivNetSession::new_with_base_url(options.clone(), credential.clone(), base_url)?;
    let page = session.fetch_current_user_homepage().await?;
    Ok(select_current_user_id(
        page.header_user_id.as_deref(),
        &page.html,
    )?)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::{
        auth::Credential,
        config::{
            BatchLayoutStrategy, DownloadConfig, DownloadMode, ResolvedDownloadOptions, SortOrder,
        },
    };

    use super::fetch_current_user_id_with_base_url;

    fn options(directory: PathBuf) -> ResolvedDownloadOptions {
        let defaults = DownloadConfig::default();
        ResolvedDownloadOptions {
            mode: DownloadMode::Illust,
            directory,
            batch_layout: BatchLayoutStrategy::Mixed,
            count: defaults.count,
            sort: SortOrder::DateDesc,
            r18: defaults.r18,
            ai: defaults.ai,
            concurrent: 1,
            timeout: 5,
            retry: 0,
            with_tags: false,
            proxy_url: None,
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn setup_can_fetch_current_user_id_from_response_header() {
        let server = MockServer::start().await;
        let temp = tempdir().unwrap();
        let credential = Credential::new("cookie").unwrap();
        let options = options(temp.path().to_path_buf());
        let base_url = Url::parse(&server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-userid", "12345678")
                    .set_body_string("<html><body>ok</body></html>"),
            )
            .mount(&server)
            .await;

        let user_id = fetch_current_user_id_with_base_url(&credential, &options, base_url)
            .await
            .unwrap();

        assert_eq!(user_id, "12345678");
    }

    #[tokio::test]
    async fn setup_can_fetch_current_user_id_from_homepage_html() {
        let server = MockServer::start().await;
        let temp = tempdir().unwrap();
        let credential = Credential::new("cookie").unwrap();
        let options = options(temp.path().to_path_buf());
        let base_url = Url::parse(&server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"<html><script>pixiv.user.id = "12345678";</script></html>"#,
                ),
            )
            .mount(&server)
            .await;

        let user_id = fetch_current_user_id_with_base_url(&credential, &options, base_url)
            .await
            .unwrap();

        assert_eq!(user_id, "12345678");
    }
}
