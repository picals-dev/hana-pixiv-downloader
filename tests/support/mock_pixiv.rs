pub fn artwork_referer(base_url: &str, illust_id: &str) -> String {
    format!("{base_url}/artworks/{illust_id}")
}

pub fn user_illustrations_referer(base_url: &str, user_id: &str) -> String {
    format!("{base_url}/users/{user_id}/illustrations")
}
