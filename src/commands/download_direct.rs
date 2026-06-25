//! `hpd download <pixiv-url>` 命令。

use crate::{
    cli::download::{DirectDownloadArgs, IllustArgs, KeywordArgs, UserArgs},
    error::{AppResult, CrawlerError},
    pixiv::url::{PixivUrlTarget, parse_pixiv_url_target},
};

pub(crate) async fn run(args: DirectDownloadArgs) -> AppResult<()> {
    let pixiv_url = args.pixiv_url.as_deref().ok_or_else(|| {
        CrawlerError::InvalidInput("请提供 Pixiv 用户、作品或标签页面 URL".to_string())
    })?;

    match parse_pixiv_url_target(pixiv_url)? {
        PixivUrlTarget::User { user_id } => {
            super::download_user::run(UserArgs {
                target: user_id,
                common: args.common,
            })
            .await
        }
        PixivUrlTarget::Illust { illust_id } => {
            super::download_illust::run(IllustArgs {
                target: illust_id,
                common: args.common,
            })
            .await
        }
        PixivUrlTarget::Keyword { query } => {
            super::download_keyword::run(KeywordArgs {
                query,
                r18: false,
                common: args.common,
            })
            .await
        }
    }
}
