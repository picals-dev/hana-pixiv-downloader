use crate::{config::config_dir, error::AppResult};

use super::{render::render_show_output, shared::load_config_snapshot};

pub(crate) async fn show() -> AppResult<()> {
    let snapshot = load_config_snapshot()?;
    let rendered = render_show_output(&snapshot, &config_dir()?);
    println!("{rendered}");

    Ok(())
}
