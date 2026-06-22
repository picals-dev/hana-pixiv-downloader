#[path = "support/mod.rs"]
mod common;

use picals_crawler::pixiv::selector::{
    select_bookmark_illust_ids, select_current_user_id, select_illust_tags,
    select_keyword_illust_ids, select_page_original_urls, select_ranking_illust_ids,
    select_user_illust_ids,
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

#[test]
fn select_keyword_ids_can_parse_fixture() {
    let value = common::read_fixture("keyword_search.json");
    let ids = select_keyword_illust_ids(&value).unwrap();

    assert_eq!(ids, vec!["146185119", "146185709"]);
}

#[test]
fn select_ranking_ids_can_parse_fixture() {
    let value = common::read_fixture("ranking.json");
    let ids = select_ranking_illust_ids(&value).unwrap();

    assert_eq!(ids, vec!["146109718", "146135045"]);
}

#[test]
fn select_bookmark_ids_can_parse_fixture() {
    let value = common::read_fixture("bookmark.json");
    let ids = select_bookmark_illust_ids(&value).unwrap();

    assert_eq!(ids, vec!["146185119", "146185709"]);
}

#[test]
fn select_current_user_id_prefers_header() {
    let html = common::read_text_fixture("homepage_logged_in.html");
    assert_eq!(
        select_current_user_id(Some("12345678"), &html).unwrap(),
        "12345678"
    );
}

#[test]
fn select_current_user_id_can_parse_homepage_html() {
    let html = common::read_text_fixture("homepage_logged_in.html");
    assert_eq!(select_current_user_id(None, &html).unwrap(), "12345678");
}

#[test]
fn select_current_user_id_rejects_missing_markers() {
    let error = select_current_user_id(None, "<html><body>empty</body></html>").unwrap_err();
    assert!(format!("{error}").contains("当前账号身份无法解析"));
}
