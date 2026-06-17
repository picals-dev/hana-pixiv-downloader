mod common;

use picals_crawler::collector::selector::{
    select_illust_tags, select_page_original_urls, select_user_illust_ids,
};

#[test]
fn select_user_ids_can_parse_fixture() {
    let value = common::read_fixture("user_profile_all.json");
    let ids = select_user_illust_ids(&value).unwrap();

    assert_eq!(ids, vec!["123456", "123457", "223456"]);
}

#[test]
fn select_page_urls_can_parse_fixture() {
    let value = common::read_fixture("illust_pages.json");
    let urls = select_page_original_urls(&value).unwrap();

    assert_eq!(
        urls,
        vec![
            "https://i.pximg.net/img-original/img/2024/01/02/03/04/05/123456_p0.png",
            "https://i.pximg.net/img-original/img/2024/01/02/03/04/05/123456_p1.png"
        ]
    );
}

#[test]
fn select_tags_prefers_translation_then_raw_tag() {
    let value = common::read_fixture("illust_detail.json");
    let tags = select_illust_tags(&value).unwrap();

    assert_eq!(tags, vec!["Hatsune Miku", "オリジナル"]);
}
