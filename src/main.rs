use anyhow::Result;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List objects in S3 bucket with date filtering
    List {
        /// S3 bucket name (defaults to "lumbrr")
        #[arg(long, default_value = "lumbrr")]
        bucket: String,

        /// Start date for filtering (ISO 8601 format: YYYY-MM-DD)
        #[arg(long)]
        start_date: String,

        /// End date for filtering (ISO 8601 format: YYYY-MM-DD)
        #[arg(long)]
        end_date: String,
    },
    /// Download an object from S3 bucket
    Download {
        /// S3 bucket name (defaults to "lumbrr")
        #[arg(long, default_value = "lumbrr")]
        bucket: String,

        /// Object key to download
        #[arg(long)]
        key: String,

        /// Output directory or file path
        #[arg(long)]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client = Client::new(&config);

    match cli.command {
        Commands::List {
            bucket,
            start_date,
            end_date,
        } => {
            list_objects(&client, &bucket, &start_date, &end_date).await?;
        }
        Commands::Download {
            bucket,
            key,
            output,
        } => {
            download_object(&client, &bucket, &key, &output).await?;
        }
    }

    Ok(())
}

async fn list_objects(
    client: &Client,
    bucket: &str,
    start_date: &str,
    end_date: &str,
) -> Result<()> {
    let start_dt = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d")?
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    let end_dt = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d")?
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_utc();

    println!(
        "Listing objects in bucket '{}' from {} to {}",
        bucket, start_date, end_date
    );

    let resp = client.list_objects_v2().bucket(bucket).send().await?;

    let objects = resp.contents();
    let mut filtered_objects = Vec::new();

    for object in objects {
        if let Some(last_modified) = object.last_modified() {
            let system_time = SystemTime::try_from(*last_modified).unwrap();
            let object_time: DateTime<Utc> = system_time.into();
            if object_time >= start_dt && object_time <= end_dt {
                filtered_objects.push(object);
            }
        }
    }

    if filtered_objects.is_empty() {
        println!("No objects found in the specified date range.");
    } else {
        println!("Found {} objects:", filtered_objects.len());
        for object in filtered_objects {
            if let Some(key) = object.key() {
                let last_modified = object
                    .last_modified()
                    .map(|dt| {
                        let system_time = SystemTime::try_from(*dt).unwrap();
                        let object_time: DateTime<Utc> = system_time.into();
                        object_time.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                    })
                    .unwrap_or_else(|| "Unknown".to_string());
                let size = object.size().unwrap_or(0);
                println!(
                    "  {} (size: {} bytes, modified: {})",
                    key, size, last_modified
                );
            }
        }
    }

    Ok(())
}

async fn download_object(client: &Client, bucket: &str, key: &str, output: &PathBuf) -> Result<()> {
    println!(
        "Downloading '{}' from bucket '{}' to '{}'",
        key,
        bucket,
        output.display()
    );

    let resp = client.get_object().bucket(bucket).key(key).send().await?;

    let data = resp.body.collect().await?;
    let bytes = data.into_bytes();

    // Create parent directories if they don't exist
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(output, &bytes)?;
    println!(
        "Successfully downloaded {} bytes to '{}'",
        bytes.len(),
        output.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_list_default_bucket() {
        let cli = Cli::try_parse_from([
            "lmbcli",
            "list",
            "--start-date",
            "2025-12-15",
            "--end-date",
            "2025-12-16",
        ])
        .unwrap();

        match cli.command {
            Commands::List {
                bucket,
                start_date,
                end_date,
            } => {
                assert_eq!(bucket, "lumbrr");
                assert_eq!(start_date, "2025-12-15");
                assert_eq!(end_date, "2025-12-16");
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_list_custom_bucket() {
        let cli = Cli::try_parse_from([
            "lmbcli",
            "list",
            "--bucket",
            "custom-bucket",
            "--start-date",
            "2025-12-15",
            "--end-date",
            "2025-12-16",
        ])
        .unwrap();

        match cli.command {
            Commands::List {
                bucket,
                start_date,
                end_date,
            } => {
                assert_eq!(bucket, "custom-bucket");
                assert_eq!(start_date, "2025-12-15");
                assert_eq!(end_date, "2025-12-16");
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_download_default_bucket() {
        let cli = Cli::try_parse_from([
            "lmbcli",
            "download",
            "--key",
            "test.log",
            "--output",
            "./downloads/",
        ])
        .unwrap();

        match cli.command {
            Commands::Download {
                bucket,
                key,
                output,
            } => {
                assert_eq!(bucket, "lumbrr");
                assert_eq!(key, "test.log");
                assert_eq!(output, PathBuf::from("./downloads/"));
            }
            _ => panic!("Expected Download command"),
        }
    }

    #[test]
    fn test_download_custom_bucket() {
        let cli = Cli::try_parse_from([
            "lmbcli",
            "download",
            "--bucket",
            "custom-bucket",
            "--key",
            "test.log",
            "--output",
            "./downloads/",
        ])
        .unwrap();

        match cli.command {
            Commands::Download {
                bucket,
                key,
                output,
            } => {
                assert_eq!(bucket, "custom-bucket");
                assert_eq!(key, "test.log");
                assert_eq!(output, PathBuf::from("./downloads/"));
            }
            _ => panic!("Expected Download command"),
        }
    }
}
