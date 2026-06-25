//! UserCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{plan_tags_and_download, sort_illust_ids},
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
    pixiv::selector::select_user_illust_ids,
};

#[derive(Debug, Clone)]
pub struct UserCrawler {
    pub artist_id: String,
    pub(crate) context: CrawlContext,
}

impl UserCrawler {
    pub fn new(
        artist_id: String,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> Self {
        options.mode = DownloadMode::User;
        Self {
            artist_id,
            context: CrawlContext::new(options, session),
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let profile = self
            .context
            .session
            .fetch_user_profile_all(&self.artist_id)
            .await?;

        let mut illust_ids = select_user_illust_ids(&profile)?;
        sort_illust_ids(&mut illust_ids, self.context.options.sort)?;
        if self.context.options.count > 0 && illust_ids.len() > self.context.options.count {
            illust_ids.truncate(self.context.options.count);
        }
        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.artist_id,
        )?;
        plan_tags_and_download(
            &self.context.session,
            illust_ids,
            &layout,
            &self.context.options,
        )
        .await
    }
}
#[cfg(test)]
mod tests {
    use crate::config::SortOrder;

    use crate::crawler::shared::sort_illust_ids;

    #[test]
    fn illust_ids_can_be_sorted_descending_by_default() {
        let mut ids = vec!["123".to_string(), "456".to_string(), "234".to_string()];
        sort_illust_ids(&mut ids, SortOrder::DateDesc).unwrap();
        assert_eq!(ids, vec!["456", "234", "123"]);
    }

    #[test]
    fn illust_ids_can_be_sorted_ascending() {
        let mut ids = vec!["123".to_string(), "456".to_string(), "234".to_string()];
        sort_illust_ids(&mut ids, SortOrder::DateAsc).unwrap();
        assert_eq!(ids, vec!["123", "234", "456"]);
    }

    #[test]
    fn illust_ids_reject_non_numeric_values() {
        let mut ids = vec!["123".to_string(), "oops".to_string()];
        let error = sort_illust_ids(&mut ids, SortOrder::DateDesc).unwrap_err();
        assert!(format!("{error:#}").contains("作品 ID 不是数字"));
    }
}
