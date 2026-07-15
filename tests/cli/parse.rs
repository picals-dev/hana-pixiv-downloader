use clap::Parser;
use hana_pixiv_downloader::cli::{Cli, Command, download::DownloadSubcommand};

#[test]
fn ranking_cli_accepts_positional_mode() {
    let cli = Cli::parse_from(["hpd", "download", "ranking", "daily"]);

    match cli.command {
        Command::Download(download) => match download.target {
            Some(DownloadSubcommand::Ranking(args)) => {
                assert_eq!(
                    args.mode,
                    Some(hana_pixiv_downloader::cli::download::RankingMode::Daily)
                );
            }
            _ => panic!("expected ranking command"),
        },
        _ => panic!("expected download command"),
    }
}

#[test]
fn keyword_cli_still_uses_query_and_r18_only() {
    let cli = Cli::parse_from(["hpd", "download", "keyword", "初音ミク", "--r18"]);

    match cli.command {
        Command::Download(download) => match download.target {
            Some(DownloadSubcommand::Keyword(args)) => {
                assert_eq!(args.query, "初音ミク");
                assert!(args.r18);
            }
            _ => panic!("expected keyword command"),
        },
        _ => panic!("expected download command"),
    }
}

#[test]
fn direct_download_cli_accepts_pixiv_url() {
    let cli = Cli::parse_from([
        "hpd",
        "download",
        "https://www.pixiv.net/users/12345678",
        "--dry-run",
    ]);

    match cli.command {
        Command::Download(download) => {
            assert!(download.target.is_none());
            assert_eq!(
                download.direct.pixiv_url.as_deref(),
                Some("https://www.pixiv.net/users/12345678")
            );
            assert!(download.direct.common.dry_run);
        }
        _ => panic!("expected download command"),
    }
}

#[test]
fn direct_download_cli_accepts_encoded_tag_url() {
    let cli = Cli::parse_from([
        "hpd",
        "download",
        "https://www.pixiv.net/tags/%E5%88%9D%E9%9F%B3%E3%83%9F%E3%82%AF/artworks",
    ]);

    match cli.command {
        Command::Download(download) => {
            assert!(download.target.is_none());
            assert_eq!(
                download.direct.pixiv_url.as_deref(),
                Some("https://www.pixiv.net/tags/%E5%88%9D%E9%9F%B3%E3%83%9F%E3%82%AF/artworks")
            );
        }
        _ => panic!("expected download command"),
    }
}

#[test]
fn retry_cli_accepts_manifest_path() {
    let cli = Cli::parse_from(["hpd", "retry", "/tmp/failures/demo.json"]);

    match cli.command {
        Command::Retry(args) => {
            assert_eq!(
                args.manifest_path,
                std::path::PathBuf::from("/tmp/failures/demo.json")
            );
        }
        _ => panic!("expected retry command"),
    }
}

#[test]
fn update_and_upgrade_parse_to_the_same_command() {
    for command in ["update", "upgrade"] {
        let cli = Cli::parse_from(["hpd", command]);
        assert!(matches!(cli.command, Command::Update));
    }
}
